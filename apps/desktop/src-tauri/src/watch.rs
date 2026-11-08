//! 后台自动整理：监听整理文件夹，文件有变动时（以及每分钟兜底一次）按规则检查。
//!
//! - 提醒模式：发现可以整理的文件时发通知，由用户打开 Foldo 确认
//! - 自动模式：直接执行可以撤销的操作；“移到回收站”仍留给用户确认
//! - 缓冲期至少 1 分钟，避免移动正在写入的文件
//! - 执行失败的文件不再反复重试，直到配置改变
//! - 每次自动整理都会记入历史，可以撤销

use crate::commands::{AppState, current_config};
use foldo_core::config::AutoMode;
use foldo_core::paths::path_key;
use foldo_core::{RuleSet, build_plan, execute};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

/// 有文件在等缓冲期或正被占用时，最晚多久再检查一次。
const INTERVAL: Duration = Duration::from_secs(60);
/// 没有文件变动、也没有等待中的文件时，兜底多久完整检查一次（以防漏掉变动，如网络磁盘）。
const IDLE_FALLBACK: Duration = Duration::from_secs(15 * 60);
/// 关闭自动整理时，兜底多久重新读一次配置（配置在应用里保存时会立即唤醒）。
const OFF_FALLBACK: Duration = Duration::from_secs(5 * 60);
/// 自动整理时的最短缓冲期。
const MIN_BUFFER: Duration = Duration::from_secs(60);
/// 收到文件变动后再等一会儿，把一连串变动合并成一次检查。
const DEBOUNCE: Duration = Duration::from_secs(3);

#[derive(Debug, Clone, Default, Serialize)]
pub struct WatchStatus {
    pub mode: &'static str,
    /// 上次检查的时间
    pub last_check: Option<String>,
    /// 等待确认的文件数（提醒模式下是全部可整理的文件，自动模式下是被留下的回收站操作）
    pub pending: usize,
    /// 最近一次自动整理的批次和完成数量
    pub last_batch: Option<i64>,
    pub last_done: usize,
    pub error: Option<String>,
}

fn mode_name(m: AutoMode) -> &'static str {
    match m {
        AutoMode::Off => "off",
        AutoMode::Notify => "notify",
        AutoMode::Run => "run",
    }
}

#[derive(Default)]
pub struct WatchState {
    pub status: Mutex<WatchStatus>,
    /// 自动执行失败过的文件，不再重试
    failed: Mutex<HashSet<String>>,
}

static WAKER: OnceLock<Mutex<Option<Sender<()>>>> = OnceLock::new();

/// 立即检查一次（配置保存后、托盘“立即整理”时调用）。
pub fn wake() {
    if let Some(tx) = WAKER
        .get()
        .and_then(|m| m.lock().ok())
        .and_then(|g| g.clone())
    {
        let _ = tx.send(());
    }
}

/// 配置改变时清空“失败过”的记录，让这些文件有机会按新规则重试。
pub fn reset_failures(app: &AppHandle) {
    if let Ok(mut f) = app.state::<WatchState>().failed.lock() {
        f.clear();
    }
}

fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app.notification().builder().title(title).body(body).show();
}

fn watch_targets(set: &RuleSet) -> Vec<(PathBuf, bool)> {
    // 只监听有启用规则的文件夹
    let mut out: Vec<(PathBuf, bool)> = set
        .folders
        .iter()
        .filter(|f| f.rules.iter().any(|&i| set.rules[i].enabled))
        .map(|f| (f.path.clone(), f.recursive))
        .collect();
    out.retain(|(p, _)| p.is_dir());
    out.sort();
    out.dedup();
    out
}

fn make_watcher(targets: &[(PathBuf, bool)], tx: Sender<()>) -> Option<RecommendedWatcher> {
    if targets.is_empty() {
        return None;
    }
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res
            && !ev.kind.is_access()
        {
            let _ = tx.send(());
        }
    })
    .ok()?;
    for (p, recursive) in targets {
        let mode = if *recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        let _ = w.watch(p, mode);
    }
    Some(w)
}

/// 启动后台线程。
pub fn spawn(app: AppHandle) {
    let (tx, rx) = mpsc::channel::<()>();
    let _ = WAKER.set(Mutex::new(Some(tx.clone())));
    std::thread::Builder::new()
        .name("foldo-watch".into())
        .spawn(move || {
            let mut _watcher: Option<RecommendedWatcher> = None;
            let mut watched: Vec<(PathBuf, bool)> = Vec::new();
            let mut last_notified = 0usize;
            loop {
                let set =
                    current_config().and_then(|c| RuleSet::from_file(c).map_err(|e| e.to_string()));
                let mode = set.as_ref().map(|s| s.auto).unwrap_or_default();

                let targets = match (&set, mode) {
                    (Ok(s), m) if m != AutoMode::Off => watch_targets(s),
                    _ => Vec::new(),
                };
                if targets != watched {
                    _watcher = make_watcher(&targets, tx.clone());
                    watched = targets;
                }

                // 下次没有文件变动时什么时候再检查
                let mut wait = OFF_FALLBACK;
                // 定时整理：到点了（或者关机时错过了）就整理一次，和自动整理一样只做可以撤销的操作
                if let Ok(s) = &set
                    && let Some(sch) = &s.schedule
                {
                    let now = chrono::Local::now();
                    let due = sch.last_due(now);
                    match read_schedule_last() {
                        // 刚开启定时整理：从下一个时间点开始
                        None => write_schedule_last(now),
                        Some(last) if last < due => {
                            write_schedule_last(now);
                            if let Ok(mut once) = current_config()
                                .and_then(|c| RuleSet::from_file(c).map_err(|e| e.to_string()))
                            {
                                once.buffer = once.buffer.max(MIN_BUFFER);
                                let mut quiet = usize::MAX;
                                let _ = cycle(&app, &once, AutoMode::Run, &mut quiet);
                            }
                        }
                        Some(_) => {}
                    }
                    let until_next = (sch.next_due(now) - now)
                        .to_std()
                        .unwrap_or(Duration::from_secs(60));
                    wait = wait.min(until_next.max(Duration::from_secs(1)));
                }
                match set {
                    Ok(mut s) if mode != AutoMode::Off => {
                        s.buffer = s.buffer.max(MIN_BUFFER);
                        wait = wait.min(
                            cycle(&app, &s, mode, &mut last_notified).unwrap_or(IDLE_FALLBACK),
                        );
                    }
                    Ok(_) => {
                        last_notified = 0;
                        set_status(&app, |st| {
                            *st = WatchStatus {
                                mode: "off",
                                ..Default::default()
                            }
                        });
                    }
                    Err(e) => set_status(&app, |st| {
                        st.mode = mode_name(mode);
                        st.error = Some(e);
                    }),
                }

                match rx.recv_timeout(wait) {
                    Ok(()) => {
                        // 合并一连串的变动
                        let until = Instant::now() + DEBOUNCE;
                        while let Some(left) = until.checked_duration_since(Instant::now()) {
                            if rx.recv_timeout(left).is_err() {
                                break;
                            }
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .expect("无法启动后台监听线程");
}

fn schedule_file() -> PathBuf {
    foldo_core::paths::default_data_dir().join("schedule-last")
}

/// 上次定时整理的时间（没有记录时为 None）。
fn read_schedule_last() -> Option<chrono::DateTime<chrono::Local>> {
    let s = std::fs::read_to_string(schedule_file()).ok()?;
    chrono::DateTime::parse_from_rfc3339(s.trim())
        .ok()
        .map(|d| d.with_timezone(&chrono::Local))
}

fn write_schedule_last(t: chrono::DateTime<chrono::Local>) {
    let path = schedule_file();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, t.to_rfc3339());
}

fn set_status(app: &AppHandle, f: impl FnOnce(&mut WatchStatus)) {
    let state = app.state::<WatchState>();
    let snapshot = {
        let Ok(mut st) = state.status.lock() else {
            return;
        };
        f(&mut st);
        st.clone()
    };
    crate::tray::refresh(app, &snapshot);
    let _ = app.emit("watch-status", snapshot);
}

/// 检查一次。返回没有文件变动时多久后需要再检查：有文件在等缓冲期或正被占用时返回等待时间，
/// 否则返回 None（只等文件变动）。
fn cycle(
    app: &AppHandle,
    set: &RuleSet,
    mode: AutoMode,
    last_notified: &mut usize,
) -> Option<Duration> {
    let now = chrono::Local::now();
    let plan = match build_plan(set, None) {
        Ok(p) => p,
        Err(e) => {
            set_status(app, |st| st.error = Some(e.to_string()));
            return Some(INTERVAL);
        }
    };
    let recheck = recheck_after(&plan, set.buffer, now);
    let failed: HashSet<String> = app
        .state::<WatchState>()
        .failed
        .lock()
        .map(|f| f.clone())
        .unwrap_or_default();
    let usable: HashSet<usize> = plan
        .ops
        .iter()
        .filter(|o| !failed.contains(&path_key(&o.origin)))
        .map(|o| o.group)
        .collect();
    let plan = plan.select_groups(&usable);

    let mut status_done = None;
    let pending = match mode {
        AutoMode::Run => {
            let (safe, held) = plan.without_irreversible();
            if !safe.is_empty() {
                let state = app.state::<AppState>();
                let result = state.with_journal(|j| execute(&safe, j, "watch"));
                let _ = state.with_journal(|j| j.prune(set.keep_history_days));
                match result {
                    Ok(report) => {
                        if let Ok(mut f) = app.state::<WatchState>().failed.lock() {
                            for (op, _) in report.failed.iter().chain(report.skipped.iter()) {
                                f.insert(path_key(&op.origin));
                            }
                        }
                        if report.done > 0 {
                            let files = safe
                                .ops
                                .iter()
                                .map(|o| o.group)
                                .collect::<HashSet<_>>()
                                .len();
                            notify(
                                app,
                                &foldo_core::t!("Foldo 已自动整理"),
                                &foldo_core::t!(
                                    "整理了 {files} 个文件，可在“历史”里撤销",
                                    files = files
                                ),
                            );
                            status_done = Some((report.batch_id, report.done));
                        }
                    }
                    Err(e) => set_status(app, |st| st.error = Some(e)),
                }
            }
            held
        }
        _ => usable.len(),
    };

    // 只在待确认的数量增加时提醒，避免每分钟都弹
    if pending > *last_notified {
        let body = if mode == AutoMode::Run {
            foldo_core::t!(
                "有 {pending} 个文件要移到回收站，请打开 Foldo 确认",
                pending = pending
            )
        } else {
            foldo_core::t!(
                "有 {pending} 个文件可以整理，打开 Foldo 查看",
                pending = pending
            )
        };
        notify(app, "Foldo", &body);
    }
    *last_notified = pending;

    set_status(app, |st| {
        st.mode = mode_name(mode);
        st.last_check = Some(now.format("%H:%M").to_string());
        st.pending = pending;
        st.error = None;
        if let Some((b, d)) = status_done {
            st.last_batch = Some(b);
            st.last_done = d;
        }
    });
    recheck
}

/// 有文件因为缓冲期或正被占用而暂缓时，算出什么时候该再看一次：
/// 缓冲期按最早到期的那个文件算（至少 5 秒、最多 1 分钟），被占用的文件 1 分钟后再看。
fn recheck_after(
    plan: &foldo_core::Plan,
    buffer: Duration,
    now: chrono::DateTime<chrono::Local>,
) -> Option<Duration> {
    use foldo_core::SkipKind;
    let mut next: Option<Duration> = None;
    let mut sooner = |d: Duration| next = Some(next.map_or(d, |n| n.min(d)));
    for s in &plan.skipped {
        match s.kind {
            SkipKind::InUse => sooner(INTERVAL),
            SkipKind::Buffer => {
                let md = std::fs::metadata(&s.path).ok();
                let newest = md
                    .as_ref()
                    .and_then(|m| {
                        let c = m.created().ok();
                        let m = m.modified().ok();
                        c.max(m)
                    })
                    .map(chrono::DateTime::<chrono::Local>::from);
                let left = newest
                    .map(|t| buffer.saturating_sub((now - t).to_std().unwrap_or_default()))
                    .unwrap_or(INTERVAL);
                sooner(left.clamp(Duration::from_secs(5), INTERVAL));
            }
            _ => {}
        }
    }
    next
}
