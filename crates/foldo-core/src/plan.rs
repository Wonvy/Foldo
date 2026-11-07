//! 生成计划：把规则展开成一份具体的操作列表。这一步只读，不改动磁盘。
//!
//! 规则按顺序生效，每个文件最多被一条规则处理（第一条命中的规则生效）。
//! 一条规则里的多个动作依次作用于文件当前的路径，例如先 rename 再 move。
//! move / copy 的目标如果是相对路径，以规则的扫描位置（location）为基准。

use crate::condition::MatchCtx;
use crate::config::OnConflict;
use crate::error::{Error, Result};
use crate::fileinfo::FileInfo;
use crate::paths::{display, is_case_only_change, path_key};
use crate::rule::{Action, Rule, RuleSet};
use crate::scan::scan;
use crate::template::TemplateCtx;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    Move,
    Copy,
    Rename,
    Trash,
}

impl OpKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OpKind::Move => "move",
            OpKind::Copy => "copy",
            OpKind::Rename => "rename",
            OpKind::Trash => "trash",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Op {
    pub rule: String,
    pub kind: OpKind,
    pub src: PathBuf,
    pub dst: Option<PathBuf>,
    /// 附加说明，例如“覆盖前移入回收站”。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Skip {
    pub rule: String,
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Debug, Default, Serialize)]
pub struct Plan {
    pub ops: Vec<Op>,
    pub skipped: Vec<Skip>,
    pub warnings: Vec<String>,
    /// 扫描到的文件数。
    pub scanned: usize,
    /// 被某条规则命中的文件数。
    pub matched: usize,
    /// 命中但已经在目标位置、无需操作的文件数。
    pub in_place: usize,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// 生成计划。`only` 指定只运行某一条规则（即使它被禁用）。
pub fn build_plan(rules: &RuleSet, only: Option<&str>) -> Result<Plan> {
    build_plan_at(rules, only, Local::now())
}

/// 同 [`build_plan`]，但可以指定“现在”的时间，便于测试。
pub fn build_plan_at(rules: &RuleSet, only: Option<&str>, now: DateTime<Local>) -> Result<Plan> {
    let selected: Vec<&Rule> = match only {
        Some(name) => vec![
            rules
                .get(name)
                .ok_or_else(|| Error::Other(format!("找不到规则「{name}」")))?,
        ],
        None => rules.rules.iter().filter(|r| r.enabled).collect(),
    };

    let mut p = Planner {
        now,
        plan: Plan::default(),
        seen: HashSet::new(),
        claimed_src: HashSet::new(),
        claimed_dst: HashSet::new(),
    };
    for rule in selected {
        p.run_rule(rule);
    }
    Ok(p.plan)
}

struct Planner {
    now: DateTime<Local>,
    plan: Plan,
    /// 扫描过的文件（多条规则扫描同一目录时只计一次）。
    seen: HashSet<String>,
    /// 已经被前面的规则处理过的源文件。
    claimed_src: HashSet<String>,
    /// 计划中已经占用的目标路径。
    claimed_dst: HashSet<String>,
}

enum Outcome {
    Planned(Vec<Op>, HashSet<String>),
    InPlace,
    Skipped(String),
}

impl Planner {
    fn run_rule(&mut self, rule: &Rule) {
        for loc in &rule.locations {
            if !loc.is_dir() {
                self.plan.warnings.push(format!(
                    "规则「{}」: 目录不存在 {}",
                    rule.name,
                    display(loc)
                ));
                continue;
            }
            let res = scan(loc, rule.recursive, rule.include_hidden);
            self.plan.warnings.extend(res.warnings);
            for f in res.files {
                let key = path_key(&f.path);
                if self.seen.insert(key.clone()) {
                    self.plan.scanned += 1;
                }
                if self.claimed_src.contains(&key) {
                    continue;
                }
                let mut ctx = MatchCtx::new(self.now);
                if !rule.condition.matches(&f, &mut ctx) {
                    continue;
                }
                self.claimed_src.insert(key);
                self.plan.matched += 1;
                match self.plan_file(rule, loc, &f, &ctx.captures) {
                    Outcome::Planned(ops, dsts) => {
                        self.claimed_dst.extend(dsts);
                        self.plan.ops.extend(ops);
                    }
                    Outcome::InPlace => self.plan.in_place += 1,
                    Outcome::Skipped(reason) => self.plan.skipped.push(Skip {
                        rule: rule.name.clone(),
                        path: f.path.clone(),
                        reason,
                    }),
                }
            }
        }
    }

    /// 为单个文件展开动作链。任何一步失败，整个文件都跳过，不会留下半套操作。
    fn plan_file(
        &self,
        rule: &Rule,
        loc: &Path,
        f: &FileInfo,
        caps: &BTreeMap<String, String>,
    ) -> Outcome {
        let mut ops: Vec<Op> = Vec::new();
        let mut local: HashSet<String> = HashSet::new();
        let mut cur = f.path.clone();

        for action in &rule.actions {
            let ctx = TemplateCtx {
                cur: &cur,
                file: f,
                captures: caps,
                now: self.now,
            };
            let (kind, dst, on_conflict) = match action {
                Action::Trash => {
                    ops.push(self.op(rule, OpKind::Trash, &cur, None, None));
                    break;
                }
                Action::Move { to, on_conflict } | Action::Copy { to, on_conflict } => {
                    let rendered = match to.render(&ctx) {
                        Ok(s) => s,
                        Err(e) => return Outcome::Skipped(e),
                    };
                    let mut dir = crate::paths::expand_home(&rendered);
                    if dir.is_relative() {
                        // 相对路径以规则的扫描位置为基准（而不是文件所在目录），
                        // 这样 recursive 时不会出现 sorted/sorted/... 的层层嵌套。
                        dir = loc.join(&dir);
                    }
                    let Some(fname) = cur.file_name() else {
                        return Outcome::Skipped("无法获取文件名".into());
                    };
                    let kind = if matches!(action, Action::Move { .. }) {
                        OpKind::Move
                    } else {
                        OpKind::Copy
                    };
                    (kind, dir.join(fname), *on_conflict)
                }
                Action::Rename { to, on_conflict } => {
                    let new_name = match to.render(&ctx) {
                        Ok(s) => s.trim().to_string(),
                        Err(e) => return Outcome::Skipped(e),
                    };
                    if new_name.is_empty()
                        || new_name == "."
                        || new_name == ".."
                        || new_name.contains(['/', '\\'])
                    {
                        return Outcome::Skipped(format!(
                            "重命名结果「{new_name}」不是合法的文件名"
                        ));
                    }
                    (OpKind::Rename, cur.with_file_name(&new_name), *on_conflict)
                }
            };

            if dst == cur {
                continue; // 已经在目标位置
            }

            let final_dst = if is_case_only_change(&cur, &dst) || !self.occupied(&dst, &local) {
                dst
            } else {
                match on_conflict {
                    OnConflict::Skip => {
                        return Outcome::Skipped(format!("目标已存在: {}", display(&dst)));
                    }
                    OnConflict::Rename => match self.free_variant(&dst, &local) {
                        Some(p) => p,
                        None => return Outcome::Skipped("找不到可用的文件名".into()),
                    },
                    OnConflict::Overwrite => {
                        let key = path_key(&dst);
                        if self.claimed_dst.contains(&key) || local.contains(&key) {
                            // 被本次计划中的另一个文件占用：不能互相覆盖，退回自动改名
                            match self.free_variant(&dst, &local) {
                                Some(p) => p,
                                None => return Outcome::Skipped("找不到可用的文件名".into()),
                            }
                        } else if dst.is_dir() {
                            return Outcome::Skipped(format!(
                                "目标是一个文件夹，无法覆盖: {}",
                                display(&dst)
                            ));
                        } else {
                            ops.push(self.op(
                                rule,
                                OpKind::Trash,
                                &dst,
                                None,
                                Some("被覆盖的旧文件".into()),
                            ));
                            dst
                        }
                    }
                }
            };

            local.insert(path_key(&final_dst));
            ops.push(self.op(rule, kind, &cur, Some(final_dst.clone()), None));
            if kind != OpKind::Copy {
                cur = final_dst;
            }
        }

        if ops.is_empty() {
            Outcome::InPlace
        } else {
            Outcome::Planned(ops, local)
        }
    }

    fn op(
        &self,
        rule: &Rule,
        kind: OpKind,
        src: &Path,
        dst: Option<PathBuf>,
        note: Option<String>,
    ) -> Op {
        Op {
            rule: rule.name.clone(),
            kind,
            src: src.to_path_buf(),
            dst,
            note,
        }
    }

    fn occupied(&self, p: &Path, local: &HashSet<String>) -> bool {
        let key = path_key(p);
        self.claimed_dst.contains(&key) || local.contains(&key) || p.symlink_metadata().is_ok()
    }

    /// `report.pdf` → `report (1).pdf`、`report (2).pdf`……
    fn free_variant(&self, dst: &Path, local: &HashSet<String>) -> Option<PathBuf> {
        let parent = dst.parent()?;
        let stem = dst.file_stem()?.to_string_lossy();
        let ext = dst.extension().map(|e| e.to_string_lossy());
        (1..10_000)
            .map(|n| match &ext {
                Some(e) => parent.join(format!("{stem} ({n}).{e}")),
                None => parent.join(format!("{stem} ({n})")),
            })
            .find(|cand| !self.occupied(cand, local))
    }
}
