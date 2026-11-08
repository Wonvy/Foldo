//! 前端可调用的命令。所有命令都是 async，在后台线程执行，扫描大目录时不会卡住界面。
//! 返回给前端的路径同时带上 `*_display`（主目录缩写成 ~），方便直接展示。

use chrono::Local;
use foldo_core::categories::{CATEGORIES, Category};
use foldo_core::condition::{Explain, MatchCtx, explain};
use foldo_core::config::{RuleDef, RuleFile, SettingsDef};
use foldo_core::journal::{BatchSummary, OpRecord};
use foldo_core::meta::FileCtx;
use foldo_core::paths::{default_data_dir, default_rules_path, display};
use foldo_core::scan::classify_file;
use foldo_core::units::Span;
use foldo_core::{
    Journal, OpKind, Plan, RuleSet, SkipKind, UndoOutcome, build_plan, build_plan_for_paths,
    execute, load_rule_file, preview_undo, undo_batch,
};
use serde::Serialize;
use std::collections::{HashSet, VecDeque};

/// 同时保留的预览数量。
const MAX_PLANS: usize = 8;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tauri::State;

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Default)]
pub struct AppState {
    journal: Mutex<Option<Journal>>,
    /// 最近生成的几份计划（主窗口和悬浮窗各自的预览互不覆盖）。
    /// 执行时必须带上对应的 plan_id，防止执行过期的预览。
    plans: Mutex<VecDeque<(u64, Plan)>>,
    plan_seq: AtomicU64,
}

impl AppState {
    pub(crate) fn with_journal<T>(
        &self,
        f: impl FnOnce(&Journal) -> foldo_core::Result<T>,
    ) -> CmdResult<T> {
        let mut guard = self.journal.lock().map_err(err)?;
        if guard.is_none() {
            *guard = Some(Journal::open_in(&default_data_dir()).map_err(err)?);
        }
        f(guard.as_ref().expect("journal 已初始化")).map_err(err)
    }

    fn store_plan(&self, plan: Plan) -> CmdResult<PlanView> {
        let id = self.plan_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let view = plan_view(id, &plan);
        let mut plans = self.plans.lock().map_err(err)?;
        plans.push_back((id, plan));
        while plans.len() > MAX_PLANS {
            plans.pop_front();
        }
        Ok(view)
    }
}

fn path_string(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

// ---------- 配置 ----------

const HEADER: &str = "# Foldo 配置文件（由图形界面维护，也可以在“设置 → 高级”里直接编辑）\n\
# 判断顺序：固定保留与排除 → 文件夹使用的规则（按规则列表从上到下，第一条命中的生效）\n\n";

#[derive(Serialize)]
pub struct ConfigView {
    /// 用户主目录，前端用来把路径显示成 ~/…
    home: String,
    path: String,
    display_path: String,
    exists: bool,
    config: Option<RuleFile>,
    /// 文件存在但有错误时的说明（需要到高级模式修正）。
    error: Option<String>,
}

fn read_config() -> ConfigView {
    let path = default_rules_path();
    let mut view = ConfigView {
        home: dirs::home_dir()
            .map(|h| path_string(&h))
            .unwrap_or_default(),
        path: path_string(&path),
        display_path: display(&path),
        exists: path.exists(),
        config: None,
        error: None,
    };
    if view.exists {
        match load_rule_file(&path) {
            Ok(file) => {
                if let Err(e) = RuleSet::from_file(file.clone()) {
                    view.error = Some(e.to_string());
                }
                view.config = Some(file);
            }
            Err(e) => view.error = Some(e.to_string()),
        }
    }
    view
}

pub(crate) fn write_config(file: &RuleFile) -> CmdResult<()> {
    RuleSet::from_file(file.clone()).map_err(err)?;
    let yaml = file.to_yaml().map_err(err)?;
    let path = default_rules_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, format!("{HEADER}{yaml}")).map_err(err)
}

pub(crate) fn current_config() -> CmdResult<RuleFile> {
    let view = read_config();
    match (view.config, view.error) {
        (Some(c), None) => Ok(c),
        (_, Some(e)) => Err(format!("配置文件有错误，请先在“设置 → 高级”中修正：{e}")),
        (None, None) => Ok(RuleFile {
            version: foldo_core::rule::SUPPORTED_VERSION,
            folders: vec![],
            protect: vec![],
            settings: SettingsDef {
                buffer: Some(Span(Duration::from_secs(600))),
                ..Default::default()
            },
            rules: vec![],
        }),
    }
}

/// 配置改变后：让后台立即按新配置检查，失败记录清零。
fn config_changed(app: &tauri::AppHandle) {
    crate::watch::reset_failures(app);
    crate::watch::wake();
}

#[tauri::command]
pub async fn get_config() -> CmdResult<ConfigView> {
    Ok(read_config())
}

/// 保存整份配置（先校验，通过才写入）。
#[tauri::command]
pub async fn save_config(app: tauri::AppHandle, config: RuleFile) -> CmdResult<ConfigView> {
    write_config(&config)?;
    config_changed(&app);
    Ok(read_config())
}

/// 把一个文件、文件夹或通配符加入固定保留。
#[tauri::command]
pub async fn add_protect(app: tauri::AppHandle, entry: String) -> CmdResult<ConfigView> {
    let mut config = current_config()?;
    if !config.protect.iter().any(|p| p == &entry) {
        config.protect.push(entry);
    }
    write_config(&config)?;
    config_changed(&app);
    Ok(read_config())
}

#[tauri::command]
pub async fn get_yaml() -> CmdResult<String> {
    let path = default_rules_path();
    if !path.exists() {
        return Ok(String::new());
    }
    std::fs::read_to_string(&path).map_err(err)
}

/// 高级模式：直接保存 YAML 源码（通过校验才写入）。
#[tauri::command]
pub async fn save_yaml(app: tauri::AppHandle, text: String) -> CmdResult<ConfigView> {
    RuleSet::from_yaml(&text).map_err(|e| format!("有错误，未保存：{e}"))?;
    let path = default_rules_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(err)?;
    }
    std::fs::write(&path, text).map_err(err)?;
    config_changed(&app);
    Ok(read_config())
}

#[tauri::command]
pub async fn categories() -> CmdResult<Vec<&'static Category>> {
    Ok(CATEGORIES.iter().collect())
}

#[derive(Serialize)]
pub struct FolderSuggestion {
    path: String,
    display: String,
    label: &'static str,
}

/// 首次使用时推荐的文件夹（存在的才列出）。
#[tauri::command]
pub async fn suggested_folders() -> CmdResult<Vec<FolderSuggestion>> {
    let candidates = [
        (dirs::download_dir(), "下载"),
        (dirs::desktop_dir(), "桌面"),
        (dirs::document_dir(), "文稿"),
    ];
    Ok(candidates
        .into_iter()
        .filter_map(|(p, label)| p.filter(|p| p.is_dir()).map(|p| (p, label)))
        .map(|(p, label)| FolderSuggestion {
            path: path_string(&p),
            display: display(&p),
            label,
        })
        .collect())
}

// ---------- 扫描与执行 ----------

#[derive(Serialize)]
pub struct OpView {
    group: usize,
    origin: String,
    origin_name: String,
    origin_display: String,
    rule: String,
    reason: String,
    category: Option<String>,
    kind: OpKind,
    src: String,
    src_display: String,
    dst: Option<String>,
    dst_display: Option<String>,
    /// 目标所在文件夹（展示“放到哪里”）。
    dst_dir: Option<String>,
    dst_dir_display: Option<String>,
    note: Option<String>,
}

#[derive(Serialize)]
pub struct SkipView {
    path: String,
    name: String,
    display: String,
    kind: SkipKind,
    label: &'static str,
    reason: String,
    rule: Option<String>,
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

fn plan_view(id: u64, plan: &Plan) -> PlanView {
    PlanView {
        plan_id: id,
        groups: plan.group_count(),
        ops: plan
            .ops
            .iter()
            .map(|o| {
                let dst_dir = o.dst.as_deref().and_then(Path::parent);
                OpView {
                    group: o.group,
                    origin: path_string(&o.origin),
                    origin_name: file_name(&o.origin),
                    origin_display: display(&o.origin),
                    rule: o.rule.clone(),
                    reason: o.reason.clone(),
                    category: o.category.clone(),
                    kind: o.kind,
                    src: path_string(&o.src),
                    src_display: display(&o.src),
                    dst: o.dst.as_deref().map(path_string),
                    dst_display: o.dst.as_deref().map(display),
                    dst_dir: dst_dir.map(path_string),
                    dst_dir_display: dst_dir.map(display),
                    note: o.note.clone(),
                }
            })
            .collect(),
        skipped: plan
            .skipped
            .iter()
            .map(|s| SkipView {
                path: path_string(&s.path),
                name: file_name(&s.path),
                display: display(&s.path),
                kind: s.kind,
                label: s.kind.label(),
                reason: s.reason.clone(),
                rule: s.rule.clone(),
            })
            .collect(),
        warnings: plan.warnings.clone(),
        scanned: plan.scanned,
        matched: plan.matched,
        in_place: plan.in_place,
    }
}

fn load_set() -> CmdResult<RuleSet> {
    RuleSet::from_file(current_config()?).map_err(err)
}

/// 按已保存的配置扫描整理文件夹。
#[tauri::command]
pub async fn scan(state: State<'_, AppState>) -> CmdResult<PlanView> {
    let set = load_set()?;
    if set.folders.is_empty() && set.rules.is_empty() {
        return Err("还没有选择要整理的文件夹".into());
    }
    let plan = build_plan(&set, None).map_err(err)?;
    state.store_plan(plan)
}

/// 处理拖入窗口的文件和文件夹。
#[tauri::command]
pub async fn scan_paths(state: State<'_, AppState>, paths: Vec<String>) -> CmdResult<PlanView> {
    let set = load_set()?;
    let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
    state.store_plan(build_plan_for_paths(&set, &paths, Local::now()))
}

/// 试运行一条尚未保存的规则：看看它会命中哪些文件。
/// `paths` 不为空时只在这些文件上试（拖入的文件），否则在规则的适用范围内扫描。
#[tauri::command]
pub async fn try_rule(
    state: State<'_, AppState>,
    rule: RuleDef,
    paths: Option<Vec<String>>,
) -> CmdResult<PlanView> {
    let mut config = current_config()?;
    let name = rule.name.clone();
    config.rules = vec![rule];
    let set = RuleSet::from_file(config).map_err(err)?;
    let plan = match paths.filter(|p| !p.is_empty()) {
        Some(paths) => {
            let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
            build_plan_for_paths(&set, &paths, Local::now())
        }
        None => build_plan(&set, Some(&name)).map_err(err)?,
    };
    state.store_plan(plan)
}

/// 预览里把某一项改到别的文件夹（只影响这一次）。
#[tauri::command]
pub async fn retarget(
    state: State<'_, AppState>,
    plan_id: u64,
    group: usize,
    dir: String,
) -> CmdResult<PlanView> {
    let mut plans = state.plans.lock().map_err(err)?;
    match plans.iter_mut().find(|(id, _)| *id == plan_id) {
        Some((id, plan)) => {
            plan.retarget_group(group, Path::new(&dir))?;
            Ok(plan_view(*id, plan))
        }
        None => Err("预览已过期，请重新扫描".into()),
    }
}

#[derive(Serialize)]
pub struct Problem {
    path: String,
    name: String,
    error: String,
}

#[derive(Serialize)]
pub struct ExecView {
    batch_id: i64,
    done: usize,
    skipped: Vec<Problem>,
    failed: Vec<Problem>,
}

#[tauri::command]
pub async fn execute_plan(
    state: State<'_, AppState>,
    plan_id: u64,
    groups: Vec<usize>,
) -> CmdResult<ExecView> {
    let plan = {
        let mut plans = state.plans.lock().map_err(err)?;
        match plans.iter().position(|(id, _)| *id == plan_id) {
            Some(i) => plans.remove(i).expect("位置有效").1,
            None => return Err("预览已过期，请重新扫描".into()),
        }
    };
    let selected: HashSet<usize> = groups.into_iter().collect();
    let subset = plan.select_groups(&selected);
    if subset.is_empty() {
        return Err("没有选中任何文件".into());
    }
    let report = state.with_journal(|j| execute(&subset, j, "gui"))?;
    let problems = |list: Vec<(foldo_core::Op, String)>| {
        list.into_iter()
            .map(|(op, e)| Problem {
                path: display(&op.origin),
                name: file_name(&op.origin),
                error: e,
            })
            .collect()
    };
    Ok(ExecView {
        batch_id: report.batch_id,
        done: report.done,
        skipped: problems(report.skipped),
        failed: problems(report.failed),
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
    name: String,
    src_display: String,
    dst_display: Option<String>,
}

#[tauri::command]
pub async fn batch_ops(state: State<'_, AppState>, id: i64) -> CmdResult<Vec<OpRecordView>> {
    let ops = state.with_journal(|j| j.ops(id))?;
    Ok(ops
        .into_iter()
        .map(|r| OpRecordView {
            name: file_name(&r.src),
            src_display: display(&r.src),
            dst_display: r.dst.as_deref().map(display),
            record: r,
        })
        .collect())
}

#[derive(Serialize)]
pub struct UndoCheckView {
    kind: String,
    name: String,
    src_display: String,
    dst_display: Option<String>,
    outcome: UndoOutcome,
    reason: Option<String>,
}

#[tauri::command]
pub async fn undo_preview(state: State<'_, AppState>, id: i64) -> CmdResult<Vec<UndoCheckView>> {
    let checks = state.with_journal(|j| preview_undo(j, id))?;
    Ok(checks
        .into_iter()
        .map(|c| UndoCheckView {
            name: file_name(&c.src),
            src_display: display(&c.src),
            dst_display: c.dst.as_deref().map(display),
            kind: c.kind,
            outcome: c.outcome,
            reason: c.reason,
        })
        .collect())
}

#[derive(Serialize)]
pub struct UndoView {
    batch_id: i64,
    restored: usize,
    failed: Vec<Problem>,
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
            .map(|(p, e)| Problem {
                name: file_name(&p),
                path: display(&p),
                error: e,
            })
            .collect(),
        in_trash: report.in_trash.iter().map(|p| display(p)).collect(),
    })
}

// ---------- 文件 ----------

// ---------- 字段、属性与规则测试 ----------

#[derive(Serialize)]
pub struct FieldsView {
    fields: Vec<&'static foldo_core::fields::FieldDef>,
    vars: &'static [foldo_core::template::VarDef],
}

/// 字段注册表与特殊变量（界面的字段选择器和“插入变量”菜单用）。
#[tauri::command]
pub async fn fields() -> CmdResult<FieldsView> {
    Ok(FieldsView {
        fields: foldo_core::fields::FIELDS.iter().collect(),
        vars: foldo_core::template::SPECIAL_VARS,
    })
}

#[derive(Serialize)]
pub struct AttrView {
    key: &'static str,
    label: &'static str,
    group: &'static str,
    unit: Option<&'static str>,
    value: Option<String>,
}

fn attr_value(v: foldo_core::fields::Value) -> String {
    use foldo_core::fields::Value;
    match v {
        Value::Size(b) => format!("{} 字节", b),
        other => other.display(),
    }
}

/// 一个文件的全部属性（读不到的值为空）。
#[tauri::command]
pub async fn inspect(path: String) -> CmdResult<Vec<AttrView>> {
    let file = FileCtx::from_path(&path).map_err(err)?;
    Ok(foldo_core::fields::inspect(&file)
        .into_iter()
        .map(|(d, v)| AttrView {
            key: d.key,
            label: d.label,
            group: d.group,
            unit: d.unit,
            value: v.map(attr_value),
        })
        .collect())
}

#[derive(Serialize)]
pub struct TestView {
    /// 规则最终是否会处理这个文件。
    matched: bool,
    /// `conditions` 列表中每一项的结果（旧写法）。
    conditions: Vec<Explain>,
    /// `when` 条件树的结果。
    when: Option<Explain>,
    /// 每条例外的结果；任何一条成立，规则就不处理这个文件。
    exceptions: Vec<Explain>,
    /// 会被放到哪里（或改成什么名字）。
    destination: Option<String>,
    /// 没有被处理时的原因。
    reason: Option<String>,
}

/// 用一个真实文件测试规则：每个条件是否成立、最终会被放到哪里。只读，不改动文件。
#[tauri::command]
pub async fn test_rule(rule: RuleDef, path: String) -> CmdResult<TestView> {
    let file = FileCtx::from_path(&path).map_err(err)?;
    let now = Local::now();
    let mut ctx = MatchCtx::new(now);
    let conditions: Vec<Explain> = rule
        .conditions
        .iter()
        .map(|c| explain(c, &file, &mut ctx))
        .collect();
    let when = rule.when.as_ref().map(|w| explain(w, &file, &mut ctx));
    let exceptions: Vec<Explain> = rule
        .exceptions
        .iter()
        .map(|c| explain(c, &file, &mut MatchCtx::new(now)))
        .collect();

    // 用这条规则单独处理这个文件，得到目标位置或跳过原因
    let mut config = current_config()?;
    let name = "__测试__".to_string();
    config.rules = vec![RuleDef {
        name: name.clone(),
        enabled: true,
        ..rule
    }];
    let set = RuleSet::from_file(config).map_err(err)?;
    let plan = build_plan_for_paths(&set, &[PathBuf::from(&path)], now);
    let destination = plan.ops.iter().rev().find_map(|o| match o.kind {
        OpKind::Trash => Some("系统回收站".to_string()),
        _ => o.dst.as_deref().map(display),
    });
    let reason = if destination.is_some() {
        None
    } else {
        plan.skipped.first().map(|s| s.reason.clone())
    };
    Ok(TestView {
        matched: destination.is_some()
            || plan
                .skipped
                .iter()
                .any(|s| s.rule.as_deref() == Some(&name)),
        conditions,
        when,
        exceptions,
        destination,
        reason,
    })
}

// ---------- 规则调试：一个文件会被哪条规则处理 ----------

#[derive(Serialize)]
pub struct RuleHit {
    index: usize,
    name: String,
    enabled: bool,
    /// 条件满足（且没有命中例外）
    matched: bool,
    /// 文件在这条规则的生效范围内
    in_scope: bool,
}

#[derive(Serialize)]
pub struct DebugView {
    path: String,
    name: String,
    display: String,
    category: Option<String>,
    /// 被保护或排除的原因（此时任何规则都不会处理它）
    excluded: Option<String>,
    rules: Vec<RuleHit>,
    /// 最终处理它的规则（第几条，从 0 开始）
    winner: Option<usize>,
    /// 处理方式：移动 / 复制 / 改名 / 移到回收站 / 保持原位
    action: Option<String>,
    destination: Option<String>,
    reason: Option<String>,
}

/// 调试：拖入的文件会被哪条规则处理、每条规则是否满足。只读，不改动文件。
/// 判断方式与拖入文件整理一致（只看启用的规则、不受生效范围限制），同时标出是否在范围内供参考。
#[tauri::command]
pub async fn debug_file(path: String) -> CmdResult<DebugView> {
    let p = PathBuf::from(&path);
    let set = RuleSet::from_file(current_config()?).map_err(err)?;
    let md = std::fs::symlink_metadata(&p).map_err(err)?;
    let mut view = DebugView {
        name: file_name(&p),
        display: display(&p),
        path: path.clone(),
        category: None,
        excluded: None,
        rules: vec![],
        winner: None,
        action: None,
        destination: None,
        reason: None,
    };
    if md.is_dir() {
        view.excluded = Some("文件夹作为整体保留，不拆分内部文件".into());
        return Ok(view);
    }
    if let Some((_, reason)) = classify_file(&p, &view.name, &md, false, &set.protect) {
        view.excluded = Some(reason);
    }
    let file = FileCtx::from_path(&p).map_err(err)?;
    view.category = file.category().map(|c| c.name.to_string());
    let now = Local::now();
    view.rules = set
        .rules
        .iter()
        .enumerate()
        .map(|(index, r)| {
            let mut ctx = MatchCtx::new(now);
            let matched = r.condition.matches(&file, &mut ctx)
                && !r
                    .exception
                    .as_ref()
                    .is_some_and(|e| e.matches(&file, &mut MatchCtx::new(now)));
            let in_scope = set.folders_using(index).any(|f| f.covers(&p));
            RuleHit {
                index,
                name: r.name.clone(),
                enabled: r.enabled,
                matched,
                in_scope,
            }
        })
        .collect();

    let plan = build_plan_for_paths(&set, std::slice::from_ref(&p), now);
    let rule_name = plan
        .ops
        .first()
        .map(|o| o.rule.clone())
        .or_else(|| plan.skipped.first().and_then(|s| s.rule.clone()));
    view.winner = rule_name.and_then(|n| set.rules.iter().position(|r| r.name == n));
    if let Some(last) = plan.ops.last() {
        view.action = Some(
            match last.kind {
                OpKind::Move => "移动到",
                OpKind::Copy => "复制到",
                OpKind::Extract => "解压到",
                OpKind::Compress => "压缩为",
                OpKind::Shortcut => "创建快捷方式到",
                OpKind::Rename => "改名为",
                OpKind::Trash => "移到回收站",
            }
            .into(),
        );
        view.destination = last.dst.as_deref().map(display);
    } else if let Some(s) = plan.skipped.first() {
        if view.winner.is_some() && s.kind == foldo_core::SkipKind::Protected {
            view.action = Some("保持原位".into());
        }
        view.reason = Some(s.reason.clone());
    }
    Ok(view)
}

// ---------- 悬浮窗 ----------

const FLOAT: &str = "float";

/// 打开或关闭悬浮窗，返回操作后是否显示。
#[tauri::command]
pub async fn toggle_float(app: tauri::AppHandle) -> CmdResult<bool> {
    toggle_float_window(&app)
}

pub(crate) fn toggle_float_window(app: &tauri::AppHandle) -> CmdResult<bool> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window(FLOAT) {
        w.close().map_err(err)?;
        return Ok(false);
    }
    tauri::WebviewWindowBuilder::new(
        app,
        FLOAT,
        tauri::WebviewUrl::App("index.html#float".into()),
    )
    .title("Foldo")
    .inner_size(84.0, 84.0)
    .resizable(false)
    .decorations(false)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible_on_all_workspaces(true)
    .build()
    .map_err(err)?;
    Ok(true)
}

#[tauri::command]
pub async fn float_visible(app: tauri::AppHandle) -> CmdResult<bool> {
    use tauri::Manager;
    Ok(app.get_webview_window(FLOAT).is_some())
}

/// 悬浮窗改变大小（展开 / 收起结果面板）。
#[tauri::command]
pub async fn float_resize(app: tauri::AppHandle, width: f64, height: f64) -> CmdResult<()> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window(FLOAT) {
        w.set_size(tauri::LogicalSize::new(width, height))
            .map_err(err)?;
    }
    Ok(())
}

/// 从悬浮窗打开主窗口。
#[tauri::command]
pub async fn show_main(app: tauri::AppHandle) -> CmdResult<()> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        w.unminimize().map_err(err)?;
        w.show().map_err(err)?;
        w.set_focus().map_err(err)?;
    }
    Ok(())
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

// ---------- 自动整理与开机自启 ----------

#[tauri::command]
pub async fn watch_status(app: tauri::AppHandle) -> CmdResult<crate::watch::WatchStatus> {
    use tauri::Manager;
    let st = app.state::<crate::watch::WatchState>();
    let s = st.status.lock().map_err(err)?.clone();
    Ok(s)
}

/// 立即检查一次。
#[tauri::command]
pub async fn watch_now() -> CmdResult<()> {
    crate::watch::wake();
    Ok(())
}

#[tauri::command]
pub async fn autostart_enabled(app: tauri::AppHandle) -> CmdResult<bool> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(err)
}

#[tauri::command]
pub async fn set_autostart(app: tauri::AppHandle, enabled: bool) -> CmdResult<bool> {
    use tauri_plugin_autostart::ManagerExt;
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(err)?;
    al.is_enabled().map_err(err)
}

// ---------- 更新 ----------

#[derive(Serialize)]
pub struct UpdateInfo {
    /// 当前版本
    pub current: String,
    /// 有新版本时为新版本号
    pub version: Option<String>,
    pub notes: Option<String>,
    pub date: Option<String>,
}

/// 检查 GitHub Releases 上有没有新版本。
#[tauri::command]
pub async fn check_update(app: tauri::AppHandle) -> CmdResult<UpdateInfo> {
    use tauri_plugin_updater::UpdaterExt;
    let current = app.package_info().version.to_string();
    let update = app
        .updater()
        .map_err(err)?
        .check()
        .await
        .map_err(|e| format!("暂时无法检查更新：{e}"))?;
    Ok(UpdateInfo {
        current,
        version: update.as_ref().map(|u| u.version.clone()),
        notes: update.as_ref().and_then(|u| u.body.clone()),
        date: update
            .as_ref()
            .and_then(|u| u.date)
            .map(|d| d.date().to_string()),
    })
}

#[derive(Clone, Serialize)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

/// 下载并安装新版本，完成后重启。下载进度通过 `update-progress` 事件发出。
#[tauri::command]
pub async fn install_update(app: tauri::AppHandle) -> CmdResult<()> {
    use tauri::Emitter;
    use tauri_plugin_updater::UpdaterExt;
    let update = app
        .updater()
        .map_err(err)?
        .check()
        .await
        .map_err(err)?
        .ok_or("已经是最新版本")?;
    let mut downloaded = 0u64;
    let progress = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| format!("更新失败：{e}"))?;
    app.restart()
}

// ---------- 规则导入导出 ----------

/// 把规则导出成单独的 YAML 文件（只有规则，不含文件夹和固定保留）。`names` 为空时导出全部。
#[tauri::command]
pub async fn export_rules(names: Vec<String>, path: String) -> CmdResult<usize> {
    let config = current_config()?;
    let rules: Vec<RuleDef> = config
        .rules
        .into_iter()
        .filter(|r| names.is_empty() || names.contains(&r.name))
        .collect();
    let count = rules.len();
    let file = RuleFile {
        version: foldo_core::rule::SUPPORTED_VERSION,
        folders: vec![],
        protect: vec![],
        settings: SettingsDef::default(),
        rules,
    };
    let yaml = file.to_yaml().map_err(err)?;
    std::fs::write(
        &path,
        format!("# Foldo 规则（{count} 条）\n# 导入：打开 Foldo → 规则 → 导入规则…，再在整理页把规则添加到文件夹\n\n{yaml}"),
    )
    .map_err(err)?;
    Ok(count)
}

/// 读取要导入的规则：可以是导出的规则文件，也可以是完整的配置文件（只取其中的规则）。
#[tauri::command]
pub async fn read_rules_file(path: String) -> CmdResult<Vec<RuleDef>> {
    let text = std::fs::read_to_string(&path).map_err(err)?;
    let file =
        foldo_core::parse_rule_file(&text).map_err(|e| format!("不是有效的规则文件：{e}"))?;
    if file.rules.is_empty() {
        return Err("这个文件里没有规则".into());
    }
    for r in &file.rules {
        foldo_core::Rule::compile(r.clone()).map_err(err)?;
    }
    Ok(file.rules)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri::async_runtime::block_on;

    /// 用真实的命令函数走一遍：读取旧版配置（自动迁移）→ 保存为新写法 → 导出规则 → 读取导入文件。
    #[test]
    fn config_migration_save_and_rule_import_export() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let inbox = root.join("inbox");
        std::fs::create_dir_all(&inbox).unwrap();
        let rules_path = root.join("rules.yaml");
        std::fs::write(
            &rules_path,
            format!(
                "version: 1\nfolders:\n  - path: '{}'\nrules:\n  - name: 文档\n    conditions: [{{ category: 文档 }}]\n    actions: [{{ move: '文档/' }}]\n  - name: 打包\n    locations: ['{}']\n    recursive: true\n    actions: [{{ compress: '{{stem}}.zip' }}]\n",
                inbox.display(),
                inbox.display()
            ),
        )
        .unwrap();
        // 只有这一个测试会改这个环境变量
        unsafe { std::env::set_var("FOLDO_RULES", &rules_path) };

        // 读取：自动迁移成 version 2，规则加入文件夹
        let view = block_on(get_config()).unwrap();
        assert!(view.error.is_none(), "{:?}", view.error);
        let cfg = view.config.unwrap();
        assert_eq!(cfg.version, 2);
        assert_eq!(cfg.folders.len(), 1);
        assert_eq!(
            cfg.folders[0].rules.as_deref(),
            Some(&["文档".to_string(), "打包".to_string()][..])
        );
        assert!(cfg.folders[0].recursive);

        // 保存：写成新写法
        write_config(&cfg).unwrap();
        let text = std::fs::read_to_string(&rules_path).unwrap();
        assert!(text.contains("version: 2"), "{text}");
        assert!(!text.contains("locations"), "{text}");
        assert!(RuleSet::load(&rules_path).is_ok());

        // 导出单条和全部规则，再读回来
        let one = root.join("one.yaml");
        assert_eq!(
            block_on(export_rules(vec!["打包".into()], path_string(&one))).unwrap(),
            1
        );
        let all = root.join("all.yaml");
        assert_eq!(
            block_on(export_rules(vec![], path_string(&all))).unwrap(),
            2
        );
        let imported = block_on(read_rules_file(path_string(&all))).unwrap();
        let names: Vec<_> = imported.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["文档", "打包"]);
        // 导出的文件只有规则，没有文件夹
        assert!(!std::fs::read_to_string(&all).unwrap().contains("folders"));
        // 完整配置文件也能当作导入来源
        assert_eq!(
            block_on(read_rules_file(path_string(&rules_path)))
                .unwrap()
                .len(),
            2
        );
        // 不是规则文件时给出明确的错误
        let bad = root.join("bad.yaml");
        std::fs::write(&bad, "hello: world\n").unwrap();
        assert!(block_on(read_rules_file(path_string(&bad))).is_err());
    }
}
