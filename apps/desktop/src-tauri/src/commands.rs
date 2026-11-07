//! 前端可调用的命令。所有命令都是 async，在后台线程执行，扫描大目录时不会卡住界面。
//! 返回给前端的路径同时带上 `*_display`（主目录缩写成 ~），方便直接展示。

use foldo_core::journal::{BatchSummary, OpRecord};
use foldo_core::paths::{default_data_dir, default_rules_path, display};
use foldo_core::{EXAMPLE_RULES, Journal, Plan, RuleSet, build_plan, execute, undo_batch};
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Default)]
pub struct AppState {
    journal: Mutex<Option<Journal>>,
    /// 最近一次生成的计划。执行时必须带上对应的 plan_id，防止执行过期的预览。
    last_plan: Mutex<Option<(u64, Plan)>>,
    plan_seq: Mutex<u64>,
}

impl AppState {
    fn with_journal<T>(&self, f: impl FnOnce(&Journal) -> foldo_core::Result<T>) -> CmdResult<T> {
        let mut guard = self.journal.lock().map_err(err)?;
        if guard.is_none() {
            *guard = Some(Journal::open_in(&default_data_dir()).map_err(err)?);
        }
        f(guard.as_ref().expect("journal 已初始化")).map_err(err)
    }
}

// ---------- 规则文件 ----------

#[derive(Serialize)]
pub struct RulesFile {
    path: String,
    display_path: String,
    exists: bool,
    text: String,
}

fn read_rules_file(path: &Path) -> CmdResult<RulesFile> {
    let exists = path.exists();
    let text = if exists {
        std::fs::read_to_string(path).map_err(err)?
    } else {
        String::new()
    };
    Ok(RulesFile {
        path: path.to_string_lossy().into_owned(),
        display_path: display(path),
        exists,
        text: text.strip_prefix('\u{feff}').unwrap_or(&text).to_string(),
    })
}

#[tauri::command]
pub async fn load_rules() -> CmdResult<RulesFile> {
    read_rules_file(&default_rules_path())
}

#[tauri::command]
pub async fn create_example_rules() -> CmdResult<RulesFile> {
    let path = default_rules_path();
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(err)?;
        }
        std::fs::write(&path, EXAMPLE_RULES).map_err(err)?;
    }
    read_rules_file(&path)
}

#[derive(Serialize)]
pub struct RuleInfo {
    name: String,
    enabled: bool,
    locations: Vec<String>,
    recursive: bool,
    actions: usize,
}

#[derive(Serialize)]
pub struct RulesSummary {
    rules: Vec<RuleInfo>,
}

fn summarize(rules: &RuleSet) -> RulesSummary {
    RulesSummary {
        rules: rules
            .rules
            .iter()
            .map(|r| RuleInfo {
                name: r.name.clone(),
                enabled: r.enabled,
                locations: r.locations.iter().map(|l| display(l)).collect(),
                recursive: r.recursive,
                actions: r.actions.len(),
            })
            .collect(),
    }
}

#[tauri::command]
pub async fn validate_rules(text: String) -> CmdResult<RulesSummary> {
    let rules = RuleSet::from_yaml(&text).map_err(err)?;
    Ok(summarize(&rules))
}

/// 只保存通过校验的规则，保证磁盘上的规则文件始终可用（以后的自动监听会依赖它）。
#[tauri::command]
pub async fn save_rules(text: String) -> CmdResult<RulesSummary> {
    let rules = RuleSet::from_yaml(&text).map_err(|e| format!("规则有错误，未保存：{e}"))?;
    let path = default_rules_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, &text).map_err(err)?;
    Ok(summarize(&rules))
}

// ---------- 预览与执行 ----------

#[derive(Serialize)]
pub struct OpView {
    group: usize,
    rule: String,
    kind: foldo_core::OpKind,
    src: String,
    dst: Option<String>,
    src_display: String,
    dst_display: Option<String>,
    note: Option<String>,
}

#[derive(Serialize)]
pub struct SkipView {
    rule: String,
    display: String,
    reason: String,
}

#[derive(Serialize)]
pub struct PlanView {
    plan_id: u64,
    groups: usize,
    ops: Vec<OpView>,
    skipped: Vec<SkipView>,
    warnings: Vec<String>,
    scanned: usize,
    matched: usize,
    in_place: usize,
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[tauri::command]
pub async fn make_plan(
    state: State<'_, AppState>,
    text: String,
    rule: Option<String>,
) -> CmdResult<PlanView> {
    let rules = RuleSet::from_yaml(&text).map_err(err)?;
    let plan = build_plan(&rules, rule.as_deref()).map_err(err)?;

    let view = PlanView {
        plan_id: 0,
        groups: plan.group_count(),
        ops: plan
            .ops
            .iter()
            .map(|o| OpView {
                group: o.group,
                rule: o.rule.clone(),
                kind: o.kind,
                src: path_string(&o.src),
                dst: o.dst.as_deref().map(path_string),
                src_display: display(&o.src),
                dst_display: o.dst.as_deref().map(display),
                note: o.note.clone(),
            })
            .collect(),
        skipped: plan
            .skipped
            .iter()
            .map(|s| SkipView {
                rule: s.rule.clone(),
                display: display(&s.path),
                reason: s.reason.clone(),
            })
            .collect(),
        warnings: plan.warnings.clone(),
        scanned: plan.scanned,
        matched: plan.matched,
        in_place: plan.in_place,
    };

    let mut seq = state.plan_seq.lock().map_err(err)?;
    *seq += 1;
    let id = *seq;
    *state.last_plan.lock().map_err(err)? = Some((id, plan));
    Ok(PlanView {
        plan_id: id,
        ..view
    })
}

#[derive(Serialize)]
pub struct Failure {
    path: String,
    error: String,
}

#[derive(Serialize)]
pub struct ExecView {
    batch_id: i64,
    done: usize,
    failed: Vec<Failure>,
}

#[tauri::command]
pub async fn execute_plan(
    state: State<'_, AppState>,
    plan_id: u64,
    groups: Vec<usize>,
) -> CmdResult<ExecView> {
    let plan = {
        let mut last = state.last_plan.lock().map_err(err)?;
        match last.take() {
            Some((id, plan)) if id == plan_id => plan,
            other => {
                *last = other;
                return Err("预览已过期，请重新生成预览".into());
            }
        }
    };
    let selected: HashSet<usize> = groups.into_iter().collect();
    let subset = plan.select_groups(&selected);
    if subset.is_empty() {
        return Err("没有选中任何文件".into());
    }
    let report = state.with_journal(|j| execute(&subset, j, "gui"))?;
    Ok(ExecView {
        batch_id: report.batch_id,
        done: report.done,
        failed: report
            .failed
            .into_iter()
            .map(|(op, e)| Failure {
                path: display(&op.src),
                error: e,
            })
            .collect(),
    })
}

// ---------- 历史与撤销 ----------

#[tauri::command]
pub async fn history(state: State<'_, AppState>, limit: usize) -> CmdResult<Vec<BatchSummary>> {
    state.with_journal(|j| j.batches(limit))
}

#[derive(Serialize)]
pub struct OpRecordView {
    #[serde(flatten)]
    record: OpRecord,
    src_display: String,
    dst_display: Option<String>,
}

#[tauri::command]
pub async fn batch_ops(state: State<'_, AppState>, id: i64) -> CmdResult<Vec<OpRecordView>> {
    let ops = state.with_journal(|j| j.ops(id))?;
    Ok(ops
        .into_iter()
        .map(|r| OpRecordView {
            src_display: display(&r.src),
            dst_display: r.dst.as_deref().map(display),
            record: r,
        })
        .collect())
}

#[derive(Serialize)]
pub struct UndoView {
    batch_id: i64,
    restored: usize,
    failed: Vec<Failure>,
    in_trash: Vec<String>,
}

#[tauri::command]
pub async fn undo(state: State<'_, AppState>, id: i64) -> CmdResult<UndoView> {
    let report = state.with_journal(|j| undo_batch(j, id))?;
    Ok(UndoView {
        batch_id: report.batch_id,
        restored: report.restored,
        failed: report
            .failed
            .into_iter()
            .map(|(p, e)| Failure {
                path: display(&p),
                error: e,
            })
            .collect(),
        in_trash: report.in_trash.iter().map(|p| display(p)).collect(),
    })
}

/// 在 Finder / 资源管理器中显示文件。文件已不存在时退而显示它所在的文件夹。
#[tauri::command]
pub async fn reveal(path: String) -> CmdResult<()> {
    let mut p = PathBuf::from(path);
    while !p.exists() {
        match p.parent() {
            Some(parent) => p = parent.to_path_buf(),
            None => return Err("路径不存在".into()),
        }
    }
    tauri_plugin_opener::reveal_item_in_dir(&p).map_err(err)
}
