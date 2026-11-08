//! 生成计划：把规则展开成一份具体的操作列表。这一步只读，不改动磁盘。
//!
//! 判断顺序：固定保留与排除 → 用户规则（按优先级）。没有规则命中的文件保持原位。
//! 每个文件最多被一条规则处理（第一条命中的规则生效）。
//! 一条规则里的多个动作依次作用于文件当前的路径，例如先 rename 再 move。
//! move / copy 的目标如果是相对路径，以规则的扫描位置（location）为基准；
//! 拖入的文件没有扫描位置，以文件所在文件夹为基准。

use crate::condition::MatchCtx;
use crate::config::{OnConflict, Target};
use crate::error::{Error, Result};
use crate::fileinfo::{FileInfo, is_hidden};
use crate::meta::FileCtx;
use crate::paths::{display, is_case_only_change, path_key};
use crate::protect::{SkipKind, is_in_use};
use crate::rule::{Action, Rule, RuleSet};
use crate::scan::{ScanResult, classify_file};
use crate::template::TemplateCtx;
use chrono::{DateTime, Local};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OpKind {
    Move,
    Copy,
    Rename,
    Trash,
    /// 解压到 `dst` 文件夹
    Extract,
    /// 压缩成 `dst` zip 文件
    Compress,
    /// 在 `dst` 创建快捷方式
    Shortcut,
}

impl OpKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OpKind::Move => "move",
            OpKind::Copy => "copy",
            OpKind::Rename => "rename",
            OpKind::Trash => "trash",
            OpKind::Extract => "extract",
            OpKind::Compress => "compress",
            OpKind::Shortcut => "shortcut",
        }
    }

    /// 执行后原文件仍在原处（后续动作继续作用于它）。
    pub fn keeps_source(self) -> bool {
        matches!(
            self,
            OpKind::Copy | OpKind::Extract | OpKind::Compress | OpKind::Shortcut
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Op {
    /// 同一个文件的动作链共享一个分组号。界面上按分组勾选，保证一条链要么全做、要么全不做。
    pub group: usize,
    /// 这一组处理的是哪个文件（动作链开始前的路径）。
    pub origin: PathBuf,
    pub rule: String,
    /// 为什么这样整理，如“规则「截图归档」”“类型分类：图片”“手动指定位置”。
    pub reason: String,
    /// 文件所属的内置分类名（如“图片”），供界面分组和显示图标。
    pub category: Option<String>,
    pub kind: OpKind,
    pub src: PathBuf,
    pub dst: Option<PathBuf>,
    /// 附加说明，例如“被覆盖的旧文件”。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// 没有进入整理计划的项目，以及原因。
#[derive(Debug, Clone, Serialize)]
pub struct Skip {
    pub path: PathBuf,
    pub kind: SkipKind,
    pub reason: String,
    /// 与哪条规则有关（目标冲突、占用等情况）。
    pub rule: Option<String>,
}

#[derive(Debug, Default, Serialize)]
pub struct Plan {
    pub ops: Vec<Op>,
    pub skipped: Vec<Skip>,
    pub warnings: Vec<String>,
    /// 扫描到的文件数（不含跳过的文件夹）。
    pub scanned: usize,
    /// 被某条规则命中的文件数。
    pub matched: usize,
    /// 命中但已经在目标位置、无需操作的文件数。
    pub in_place: usize,
    /// 执行后可以清理空文件夹的范围：只删除这些文件夹内部（不含它们自己）因整理而变空的文件夹。
    /// 为空表示不清理（如拖入的文件，或设置了保留空文件夹）。
    pub cleanup_roots: Vec<PathBuf>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// 分组号上限（分组号从 0 开始；修改目标后分组号不变）。
    pub fn group_count(&self) -> usize {
        self.ops.iter().map(|o| o.group + 1).max().unwrap_or(0)
    }

    /// 只保留选中分组的操作，用于“预览后只执行勾选的文件”。
    pub fn select_groups(&self, groups: &HashSet<usize>) -> Plan {
        Plan {
            ops: self
                .ops
                .iter()
                .filter(|o| groups.contains(&o.group))
                .cloned()
                .collect(),
            skipped: self.skipped.clone(),
            warnings: self.warnings.clone(),
            scanned: self.scanned,
            matched: self.matched,
            in_place: self.in_place,
            cleanup_roots: self.cleanup_roots.clone(),
        }
    }

    /// 去掉包含不可撤销操作（移到回收站）的文件，返回 (剩下的计划, 被留下待确认的文件数)。
    /// 自动整理只执行可以撤销的操作。
    pub fn without_irreversible(&self) -> (Plan, usize) {
        let risky: HashSet<usize> = self
            .ops
            .iter()
            .filter(|o| o.kind == OpKind::Trash)
            .map(|o| o.group)
            .collect();
        let keep: HashSet<usize> = self
            .ops
            .iter()
            .map(|o| o.group)
            .filter(|g| !risky.contains(g))
            .collect();
        (self.select_groups(&keep), risky.len())
    }

    /// 按原因统计跳过的项目数。
    pub fn skip_summary(&self) -> Vec<(SkipKind, usize)> {
        let mut m: BTreeMap<SkipKind, usize> = BTreeMap::new();
        for s in &self.skipped {
            *m.entry(s.kind).or_default() += 1;
        }
        m.into_iter().collect()
    }

    /// 把某一组改为“移动到 `dir`”（用户在预览里手动改位置）。
    /// 原来的动作链被替换为一次移动；同名时自动改名，不覆盖。返回最终的目标路径。
    pub fn retarget_group(
        &mut self,
        group: usize,
        dir: &Path,
    ) -> std::result::Result<PathBuf, String> {
        let first = self
            .ops
            .iter()
            .position(|o| o.group == group)
            .ok_or_else(|| crate::t!("找不到这一项"))?;
        if !dir.is_absolute() {
            return Err(crate::t!("目标文件夹必须是绝对路径"));
        }
        let template = self.ops[first].clone();
        let origin = template.origin.clone();
        let name = origin
            .file_name()
            .ok_or_else(|| crate::t!("无法获取文件名"))?;
        let target = dir.join(name);
        if target == origin {
            return Err(crate::t!("文件已经在这个文件夹里"));
        }

        let others: HashSet<String> = self
            .ops
            .iter()
            .filter(|o| o.group != group)
            .filter_map(|o| o.dst.as_deref().map(path_key))
            .collect();
        let occupied = |p: &Path| others.contains(&path_key(p)) || p.symlink_metadata().is_ok();
        let dst = if occupied(&target) {
            free_variant(&target, occupied).ok_or_else(|| crate::t!("找不到可用的文件名"))?
        } else {
            target
        };

        self.ops.retain(|o| o.group != group);
        self.ops.insert(
            first.min(self.ops.len()),
            Op {
                group,
                origin: origin.clone(),
                rule: "手动指定".into(),
                reason: crate::t!("手动指定位置"),
                category: template.category,
                kind: OpKind::Move,
                src: origin,
                dst: Some(dst.clone()),
                note: None,
            },
        );
        Ok(dst)
    }
}

/// 生成计划。`only` 指定只运行某一条规则（即使它被禁用）。
pub fn build_plan(rules: &RuleSet, only: Option<&str>) -> Result<Plan> {
    build_plan_at(rules, only, Local::now())
}

/// 同 [`build_plan`]，但可以指定“现在”的时间，便于测试。
///
/// 逐个文件夹扫描，每个文件按该文件夹自己的规则顺序匹配，第一条命中的规则处理它。
/// `only` 只运行指定的规则（即使它被禁用），用在使用它的文件夹上；还没有文件夹使用它时，试用在全部文件夹上。
pub fn build_plan_at(set: &RuleSet, only: Option<&str>, now: DateTime<Local>) -> Result<Plan> {
    build_plan_with(set, only, now, &crate::progress::ScanControl::default())
}

/// 同 [`build_plan_at`]，并更新进度；在 `ctrl` 上请求取消后返回“已取消”错误。
pub fn build_plan_with(
    set: &RuleSet,
    only: Option<&str>,
    now: DateTime<Local>,
    ctrl: &crate::progress::ScanControl,
) -> Result<Plan> {
    let only = match only {
        Some(name) => Some(
            set.index_of(name)
                .ok_or_else(|| Error::Other(crate::t!("找不到规则「{name}」", name = name)))?,
        ),
        None => None,
    };
    let unassigned = only.is_some_and(|i| set.folders_using(i).next().is_none());

    let mut p = Planner::new(set, now);
    p.ctrl = Some(ctrl);
    if set.clean_empty_folders {
        p.plan.cleanup_roots = set.folders.iter().map(|f| f.path.clone()).collect();
    }
    // 先把全部整理文件夹扫描一遍，“重复文件”可以跨文件夹判断
    p.prescan(&set.folders);
    for folder in &set.folders {
        let rules: Vec<&Rule> = match only {
            Some(i) if unassigned || folder.rules.contains(&i) => vec![&set.rules[i]],
            Some(_) => continue,
            None => folder
                .rules
                .iter()
                .map(|&i| &set.rules[i])
                .filter(|r| r.enabled)
                .collect(),
        };
        if !folder.path.is_dir() {
            let w = crate::t!("目录不存在：{a0}", a0 = display(&folder.path));
            if !p.plan.warnings.contains(&w) {
                p.plan.warnings.push(w);
            }
            continue;
        }
        // 没有规则的文件夹也要扫描：里面的文件列为“没有匹配的规则”
        let res = p.scan_cached(&folder.path, folder.recursive, folder.include_hidden);
        // 先处理子文件夹（文件夹规则）；被文件夹规则处理的子文件夹，里面的文件不再单独处理
        let mut taken: Vec<String> = Vec::new();
        for d in &res.folders {
            let mut handled = false;
            for rule in rules.iter().filter(|r| r.target == Target::Folder) {
                if p.consider(rule, Some(&folder.path), d) {
                    handled = true;
                    break;
                }
            }
            if handled && p.claimed_src.contains(&path_key(&d.path)) {
                taken.push(path_key(&d.path));
            } else if !handled && !folder.recursive {
                p.add_skip(
                    &d.path,
                    SkipKind::Folder,
                    crate::t!("已有文件夹作为整体保留，不拆分内部文件"),
                    None,
                );
            }
        }
        let inside_taken = |f: &FileCtx| {
            let k = path_key(&f.path);
            taken
                .iter()
                .any(|t| k.starts_with(t.as_str()) && k[t.len()..].starts_with(['/', '\\']))
        };
        for f in res.files.iter().filter(|f| !inside_taken(f)) {
            if ctrl.cancelled() {
                return Err(Error::Other(crate::t!("已取消扫描")));
            }
            ctrl.add_checked();
            p.see(f);
            for rule in rules.iter().filter(|r| r.target == Target::File) {
                if p.consider(rule, Some(&folder.path), f) {
                    break;
                }
            }
        }
    }
    if ctrl.cancelled() {
        return Err(Error::Other(crate::t!("已取消扫描")));
    }
    Ok(p.finish())
}

/// 处理拖入的文件（类似 DropIt）：不受规则适用范围限制；没有匹配的文件标记为“没有匹配的规则”。
pub fn build_plan_for_paths(set: &RuleSet, paths: &[PathBuf], now: DateTime<Local>) -> Plan {
    let mut p = Planner::new(set, now);
    p.report_in_place = true;
    let rules: Vec<&Rule> = set.rules.iter().filter(|r| r.enabled).collect();
    let mut files: Vec<FileCtx> = Vec::new();
    for path in paths {
        let Ok(md) = path.symlink_metadata() else {
            p.plan
                .warnings
                .push(crate::t!("找不到 {a0}", a0 = display(path)));
            continue;
        };
        if path.to_str().is_none() {
            p.plan.warnings.push(crate::t!(
                "跳过文件名不是 UTF-8 编码的项目: {a0}",
                a0 = path.display()
            ));
            continue;
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if md.is_dir() {
            if let Some(reason) = set.protect.check(path, &name) {
                p.add_skip(path, SkipKind::Protected, reason, None);
                continue;
            }
            let mut d: FileCtx = FileInfo::new(path.clone(), &md, is_hidden(&name, &md)).into();
            d.set_categories(&set.categories);
            let handled = rules
                .iter()
                .filter(|r| r.target == Target::Folder)
                .any(|r| p.consider(r, None, &d));
            if !handled {
                p.add_skip(
                    path,
                    SkipKind::Folder,
                    crate::t!("文件夹作为整体保留，不拆分内部文件（可以新建处理文件夹的规则）"),
                    None,
                );
            }
            continue;
        }
        if !md.is_file() {
            p.add_skip(
                path,
                SkipKind::Invalid,
                crate::t!("仅支持普通文件或文件夹，不跟随符号链接"),
                None,
            );
            continue;
        }
        if let Some((kind, reason)) = classify_file(path, &name, &md, false, &set.protect) {
            p.add_skip(path, kind, reason, None);
            continue;
        }
        files.push(FileInfo::new(path.clone(), &md, is_hidden(&name, &md)).into());
    }
    // 拖入的文件之间也能判断重复
    crate::scan::attach_peers(&mut files);
    for f in &mut files {
        f.set_categories(&set.categories);
    }
    for f in &files {
        p.see(f);
        for rule in rules.iter().filter(|r| r.target == Target::File) {
            if p.consider(rule, None, f) {
                break;
            }
        }
    }
    p.finish()
}

struct Planner<'a> {
    set: &'a RuleSet,
    /// Explicit drops should explain every item, even if no operation is needed.
    report_in_place: bool,
    /// 进度和取消（只有整理文件夹的扫描用）
    ctrl: Option<&'a crate::progress::ScanControl>,
    now: DateTime<Local>,
    plan: Plan,
    /// 扫描到的文件（多条规则扫描同一目录时只计一次），用于最后列出“没有匹配的规则”。
    seen: HashMap<String, PathBuf>,
    skipped_keys: HashSet<String>,
    /// 已经被前面的规则处理过的源文件。
    claimed_src: HashSet<String>,
    /// 计划中已经占用的目标路径。
    claimed_dst: HashSet<String>,
    scan_cache: HashMap<(String, bool, bool), Rc<ScanResult>>,
}

enum Outcome {
    Planned(Vec<Op>, HashSet<String>),
    InPlace,
    Skipped(SkipKind, String),
}

impl<'a> Planner<'a> {
    fn new(set: &'a RuleSet, now: DateTime<Local>) -> Self {
        Self {
            set,
            report_in_place: false,
            ctrl: None,
            now,
            plan: Plan::default(),
            seen: HashMap::new(),
            skipped_keys: HashSet::new(),
            claimed_src: HashSet::new(),
            claimed_dst: HashSet::new(),
            scan_cache: HashMap::new(),
        }
    }

    fn finish(mut self) -> Plan {
        let mut unmatched: Vec<PathBuf> = self
            .seen
            .iter()
            .filter(|(k, _)| !self.claimed_src.contains(*k) && !self.skipped_keys.contains(*k))
            .map(|(_, p)| p.clone())
            .collect();
        unmatched.sort();
        for path in unmatched {
            self.plan.skipped.push(Skip {
                path,
                kind: SkipKind::NoMatch,
                reason: crate::t!("没有匹配的规则，保持原位"),
                rule: None,
            });
        }
        self.plan.scanned = self.seen.len();
        self.plan
    }

    fn add_skip(&mut self, path: &Path, kind: SkipKind, reason: String, rule: Option<&Rule>) {
        if self.skipped_keys.insert(path_key(path)) {
            self.plan.skipped.push(Skip {
                path: path.to_path_buf(),
                kind,
                reason,
                rule: rule.map(|r| r.name.clone()),
            });
        }
    }

    fn see(&mut self, f: &FileCtx) {
        self.seen
            .entry(path_key(&f.path))
            .or_insert_with(|| f.path.clone());
    }

    /// 扫描全部整理文件夹，并让所有文件互相知道彼此（判断重复文件时跨文件夹比较）。
    fn prescan(&mut self, folders: &[crate::rule::Folder]) {
        let mut fresh: Vec<((String, bool, bool), ScanResult)> = Vec::new();
        for f in folders.iter().filter(|f| f.path.is_dir()) {
            let key = (path_key(&f.path), f.recursive, f.include_hidden);
            if self.scan_cache.contains_key(&key) || fresh.iter().any(|(k, _)| *k == key) {
                continue;
            }
            fresh.push((key, self.scan_raw(&f.path, f.recursive, f.include_hidden)));
        }
        // 嵌套的文件夹可能扫到同一个文件，只算一次
        let mut seen = HashSet::new();
        let all: Vec<(&Path, u64, Option<DateTime<Local>>)> = fresh
            .iter()
            .flat_map(|(_, r)| r.files.iter())
            .filter(|f| seen.insert(path_key(&f.path)))
            .map(|f| (f.path.as_path(), f.size, f.modified))
            .collect();
        let peers = Rc::new(crate::meta::Peers::new(all));
        for (key, mut res) in fresh {
            for f in &mut res.files {
                f.set_peers(peers.clone());
            }
            self.store_scan(key, res);
        }
    }

    fn scan_raw(&self, loc: &Path, recursive: bool, include_hidden: bool) -> ScanResult {
        let mut res =
            crate::scan::scan_with(loc, recursive, include_hidden, &self.set.protect, self.ctrl);
        for f in res.files.iter_mut().chain(res.folders.iter_mut()) {
            f.set_categories(&self.set.categories);
        }
        res
    }

    fn scan_cached(&mut self, loc: &Path, recursive: bool, include_hidden: bool) -> Rc<ScanResult> {
        let key = (path_key(loc), recursive, include_hidden);
        if let Some(r) = self.scan_cache.get(&key) {
            return r.clone();
        }
        let res = self.scan_raw(loc, recursive, include_hidden);
        self.store_scan(key, res)
    }

    fn store_scan(&mut self, key: (String, bool, bool), res: ScanResult) -> Rc<ScanResult> {
        let res = Rc::new(res);
        self.plan.warnings.extend(res.warnings.iter().cloned());
        for (path, kind, reason) in &res.skipped {
            self.add_skip(path, *kind, reason.clone(), None);
        }
        self.scan_cache.insert(key, res.clone());
        res
    }

    fn within_buffer(&self, f: &FileCtx) -> bool {
        if self.set.buffer.is_zero() {
            return false;
        }
        let mut newest = match (f.created, f.modified) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        // 文件夹：里面有刚加进来或刚改过的东西（比如还在解压、下载）也先不动
        if let Some(inner) = f.folder().and_then(|m| m.last_change()) {
            newest = newest.max(Some(inner));
        }
        newest.is_some_and(|t| (self.now - t).to_std().unwrap_or_default() < self.set.buffer)
    }

    /// 用一条规则处理一个文件。返回 true 表示这个文件已经有了结论（已计划或已跳过）。
    fn consider(&mut self, rule: &Rule, base: Option<&Path>, f: &FileCtx) -> bool {
        let key = path_key(&f.path);
        if self.claimed_src.contains(&key) || self.skipped_keys.contains(&key) {
            return true;
        }
        if self.within_buffer(f) {
            let span = crate::units::Span(self.set.buffer);
            self.add_skip(
                &f.path,
                SkipKind::Buffer,
                crate::t!("最近 {span} 内新增或修改，暂不整理", span = span),
                None,
            );
            return true;
        }
        let mut ctx = MatchCtx::new(self.now);
        if !rule.condition.matches(f, &mut ctx) {
            return false;
        }
        if let Some(ex) = &rule.exception
            && ex.matches(f, &mut MatchCtx::new(self.now))
        {
            return false;
        }
        if !f.is_dir && is_in_use(&f.path) {
            self.add_skip(
                &f.path,
                SkipKind::InUse,
                crate::t!("文件正被其他程序使用，稍后再整理"),
                Some(rule),
            );
            return true;
        }

        self.claimed_src.insert(key);
        self.plan.matched += 1;
        let base = base
            .map(Path::to_path_buf)
            .or_else(|| f.path.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        match self.plan_file(rule, &base, f, &ctx.captures) {
            Outcome::Planned(ops, dsts) => {
                let group = self.plan.group_count();
                self.claimed_dst.extend(dsts);
                self.plan
                    .ops
                    .extend(ops.into_iter().map(|op| Op { group, ..op }));
            }
            Outcome::InPlace => {
                self.plan.in_place += 1;
                if self.report_in_place {
                    self.add_skip(
                        &f.path,
                        SkipKind::InPlace,
                        crate::t!("已在目标位置，无需整理"),
                        Some(rule),
                    );
                }
            }
            Outcome::Skipped(kind, reason) => {
                // 已计入 claimed_src，这里直接记录，不走去重
                self.skipped_keys.insert(path_key(&f.path));
                self.plan.skipped.push(Skip {
                    path: f.path.clone(),
                    kind,
                    reason,
                    rule: Some(rule.name.clone()),
                });
            }
        }
        true
    }

    /// 为单个文件展开动作链。任何一步失败，整个文件都跳过，不会留下半套操作。
    fn plan_file(
        &self,
        rule: &Rule,
        base: &Path,
        f: &FileCtx,
        caps: &BTreeMap<String, String>,
    ) -> Outcome {
        let mut ops: Vec<Op> = Vec::new();
        let mut local: HashSet<String> = HashSet::new();
        let mut cur = f.path.clone();
        let category = if f.is_dir {
            Some("文件夹".to_string())
        } else {
            f.category().map(|c| c.name.to_string())
        };
        let op = |kind: OpKind, src: &Path, dst: Option<PathBuf>, note: Option<String>| Op {
            group: 0, // 计划汇总时再分配
            origin: f.path.clone(),
            rule: rule.name.clone(),
            reason: rule.reason.clone(),
            category: category.clone(),
            kind,
            src: src.to_path_buf(),
            dst,
            note,
        };

        for action in &rule.actions {
            let ctx = TemplateCtx {
                cur: &cur,
                file: f,
                captures: caps,
                now: self.now,
            };
            let (kind, dst, on_conflict) = match action {
                Action::Keep => {
                    return Outcome::Skipped(
                        SkipKind::Protected,
                        crate::t!("{a0}：保持原位", a0 = rule.reason),
                    );
                }
                Action::Trash => {
                    ops.push(op(OpKind::Trash, &cur, None, None));
                    break;
                }
                Action::Move { to, on_conflict } | Action::Copy { to, on_conflict } => {
                    let rendered = match to.render(&ctx) {
                        Ok(s) => s,
                        Err(e) => return Outcome::Skipped(SkipKind::Invalid, e),
                    };
                    let mut dir = crate::paths::expand_home(&rendered);
                    if dir.is_relative() {
                        // 相对路径以规则的扫描位置为基准（而不是文件所在目录），
                        // 这样 recursive 时不会出现 sorted/sorted/... 的层层嵌套。
                        dir = base.join(&dir);
                    }
                    let Some(fname) = cur.file_name() else {
                        return Outcome::Skipped(SkipKind::Invalid, crate::t!("无法获取文件名"));
                    };
                    let kind = if matches!(action, Action::Move { .. }) {
                        OpKind::Move
                    } else {
                        OpKind::Copy
                    };
                    (kind, dir.join(fname), *on_conflict)
                }
                Action::Extract { to, on_conflict } => {
                    if crate::fsops::archive_kind(&cur).is_none() {
                        return Outcome::Skipped(
                            SkipKind::Invalid,
                            crate::t!(
                                "{reason}：目前只能解压 zip、7z、rar、tar、tar.gz 文件",
                                reason = rule.reason
                            ),
                        );
                    }
                    let dir = match render_near(to, &ctx, &cur) {
                        Ok(d) => d,
                        Err(e) => return Outcome::Skipped(SkipKind::Invalid, e),
                    };
                    (OpKind::Extract, dir, *on_conflict)
                }
                Action::Compress { to, on_conflict } => {
                    let mut dst = match render_near(to, &ctx, &cur) {
                        Ok(d) => d,
                        Err(e) => return Outcome::Skipped(SkipKind::Invalid, e),
                    };
                    let rendered_dir = to.render(&ctx).is_ok_and(|s| s.ends_with(['/', '\\']));
                    if rendered_dir {
                        // 文件夹名里的点不是扩展名
                        let stem = if f.is_dir {
                            cur.file_name().unwrap_or_default().to_string_lossy()
                        } else {
                            cur.file_stem().unwrap_or_default().to_string_lossy()
                        };
                        dst = dst.join(format!("{stem}.zip"));
                    } else if !dst
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("zip"))
                    {
                        dst.as_mut_os_string().push(".zip");
                    }
                    (OpKind::Compress, dst, *on_conflict)
                }
                Action::Shortcut { to, on_conflict } => {
                    let rendered = match to.render(&ctx) {
                        Ok(s) => s,
                        Err(e) => return Outcome::Skipped(SkipKind::Invalid, e),
                    };
                    let mut dir = crate::paths::expand_home(&rendered);
                    if dir.is_relative() {
                        dir = base.join(&dir);
                    }
                    let Some(fname) = cur.file_name() else {
                        return Outcome::Skipped(SkipKind::Invalid, crate::t!("无法获取文件名"));
                    };
                    let mut name = fname.to_os_string();
                    if cfg!(windows) {
                        name.push(".lnk");
                    }
                    (OpKind::Shortcut, dir.join(name), *on_conflict)
                }
                Action::Rename { to, on_conflict } => {
                    let new_name = match to.render(&ctx) {
                        Ok(s) => s.trim().to_string(),
                        Err(e) => return Outcome::Skipped(SkipKind::Invalid, e),
                    };
                    if new_name.is_empty()
                        || new_name == "."
                        || new_name == ".."
                        || new_name.contains(['/', '\\'])
                    {
                        return Outcome::Skipped(
                            SkipKind::Invalid,
                            crate::t!(
                                "重命名结果「{new_name}」不是合法的文件名",
                                new_name = new_name
                            ),
                        );
                    }
                    (OpKind::Rename, cur.with_file_name(&new_name), *on_conflict)
                }
            };

            if dst == cur {
                continue; // 已经在目标位置
            }
            // 保留原文件的动作（复制、解压、压缩、快捷方式）做过一次后结果已经在那里：
            // 视为已完成，否则每次扫描（以及自动整理每分钟）都会再生成一份
            if kind.keeps_source()
                && !local.contains(&path_key(&dst))
                && already_done(kind, &cur, &dst)
            {
                continue;
            }

            let occupied = |p: &Path| self.occupied(p, &local);
            let final_dst = if is_case_only_change(&cur, &dst) || !occupied(&dst) {
                dst
            } else {
                match on_conflict {
                    OnConflict::Skip => {
                        return Outcome::Skipped(
                            SkipKind::Conflict,
                            crate::t!("目标位置已有同名文件：{a0}", a0 = display(&dst)),
                        );
                    }
                    OnConflict::Rename => match free_variant(&dst, occupied) {
                        Some(p) => p,
                        None => {
                            return Outcome::Skipped(
                                SkipKind::Conflict,
                                crate::t!("找不到可用的文件名"),
                            );
                        }
                    },
                    OnConflict::Overwrite => {
                        let key = path_key(&dst);
                        if self.claimed_dst.contains(&key) || local.contains(&key) {
                            // 被本次计划中的另一个文件占用：不能互相覆盖，退回自动改名
                            match free_variant(&dst, occupied) {
                                Some(p) => p,
                                None => {
                                    return Outcome::Skipped(
                                        SkipKind::Conflict,
                                        crate::t!("找不到可用的文件名"),
                                    );
                                }
                            }
                        } else if dst.is_dir() {
                            return Outcome::Skipped(
                                SkipKind::Conflict,
                                crate::t!("目标是一个文件夹，无法覆盖：{a0}", a0 = display(&dst)),
                            );
                        } else {
                            ops.push(op(
                                OpKind::Trash,
                                &dst,
                                None,
                                Some(crate::t!("被覆盖的旧文件")),
                            ));
                            dst
                        }
                    }
                }
            };

            local.insert(path_key(&final_dst));
            ops.push(op(kind, &cur, Some(final_dst.clone()), None));
            if !kind.keeps_source() {
                cur = final_dst;
            }
        }

        if ops.is_empty() {
            Outcome::InPlace
        } else {
            Outcome::Planned(ops, local)
        }
    }

    fn occupied(&self, p: &Path, local: &HashSet<String>) -> bool {
        let key = path_key(p);
        self.claimed_dst.contains(&key) || local.contains(&key) || p.symlink_metadata().is_ok()
    }
}

/// 复制的副本大小相同，或解压 / 压缩 / 快捷方式的结果已经存在。
fn already_done(kind: OpKind, cur: &Path, dst: &Path) -> bool {
    match kind {
        OpKind::Copy => match (cur.metadata(), dst.metadata()) {
            (Ok(a), Ok(b)) if a.is_dir() => b.is_dir(),
            (Ok(a), Ok(b)) => b.is_file() && a.len() == b.len(),
            _ => false,
        },
        _ => dst.symlink_metadata().is_ok(),
    }
}

/// 渲染目标路径；相对路径以文件当前所在的文件夹为准（解压、压缩用）。
fn render_near(
    to: &crate::template::Template,
    ctx: &TemplateCtx,
    cur: &Path,
) -> Result<PathBuf, String> {
    let rendered = to.render(ctx)?;
    let rendered = rendered.trim_end_matches(['/', '\\']);
    if rendered.is_empty() {
        return Err(crate::t!("目标路径为空"));
    }
    let p = crate::paths::expand_home(rendered);
    Ok(if p.is_relative() {
        cur.parent().unwrap_or(Path::new("")).join(p)
    } else {
        p
    })
}

/// `report.pdf` → `report (1).pdf`、`report (2).pdf`……
fn free_variant(dst: &Path, occupied: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    let parent = dst.parent()?;
    let stem = dst.file_stem()?.to_string_lossy();
    let ext = dst.extension().map(|e| e.to_string_lossy());
    (1..10_000)
        .map(|n| match &ext {
            Some(e) => parent.join(format!("{stem} ({n}).{e}")),
            None => parent.join(format!("{stem} ({n})")),
        })
        .find(|cand| !occupied(cand))
}
