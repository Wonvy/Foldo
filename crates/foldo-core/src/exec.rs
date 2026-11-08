//! 按计划执行操作，并逐条写入操作日志。单个操作出问题不会中断整个批次。
//!
//! 结果分三类：成功；跳过（执行时才出现的新冲突，如目标位置刚出现同名文件、源文件已被移走）；
//! 失败（权限不足、磁盘错误等）。同一个文件的动作链中有一步没完成，后面的步骤一律跳过。

use crate::error::Result;
use crate::fsops::{self, exists};
use crate::journal::{Journal, KIND_MKDIR, status};
use crate::paths::{is_case_only_change, path_key};
use crate::plan::{Op, OpKind, Plan};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct ExecReport {
    pub batch_id: i64,
    pub done: usize,
    pub skipped: Vec<(Op, String)>,
    pub failed: Vec<(Op, String)>,
    /// 因整理而变空、已删除的文件夹。
    pub removed_dirs: Vec<PathBuf>,
}

enum Problem {
    Skip(String),
    Fail(String),
}

/// 执行计划。`source` 记录这次执行从哪里触发（如 `cli`、`gui`、`watch`）。
pub fn execute(plan: &Plan, journal: &Journal, source: &str) -> Result<ExecReport> {
    let batch = journal.begin_batch(source)?;
    let mut report = ExecReport {
        batch_id: batch,
        ..Default::default()
    };
    let mut broken_groups: HashSet<usize> = HashSet::new();
    let mut seq: i64 = 0;
    // 有文件被移走的文件夹，执行完后检查是否变空
    let mut vacated: Vec<PathBuf> = Vec::new();

    for op in &plan.ops {
        if broken_groups.contains(&op.group) {
            report
                .skipped
                .push((op.clone(), "同一文件的前一步没有完成".into()));
            continue;
        }
        if let Some(dst) = &op.dst {
            match fsops::ensure_parent(dst) {
                Ok(created) => {
                    for dir in created {
                        seq += 1;
                        let id =
                            journal.begin_op(batch, seq, &op.rule, KIND_MKDIR, &dir, None, None)?;
                        journal.finish_op(id, status::DONE, None)?;
                    }
                }
                Err(e) => {
                    broken_groups.insert(op.group);
                    report
                        .failed
                        .push((op.clone(), format!("无法创建目标文件夹：{e}")));
                    continue;
                }
            }
        }

        seq += 1;
        let size = std::fs::metadata(&op.src).map(|m| m.len()).ok();
        let id = journal.begin_op(
            batch,
            seq,
            &op.rule,
            op.kind.as_str(),
            &op.src,
            op.dst.as_deref(),
            size,
        )?;
        match apply(op) {
            Ok(()) => {
                journal.finish_op(id, status::DONE, None)?;
                report.done += 1;
                if matches!(op.kind, OpKind::Move | OpKind::Rename | OpKind::Trash)
                    && let Some(parent) = op.src.parent()
                {
                    vacated.push(parent.to_path_buf());
                }
            }
            Err(Problem::Skip(e)) => {
                broken_groups.insert(op.group);
                journal.finish_op(id, status::SKIPPED, Some(&e))?;
                report.skipped.push((op.clone(), e));
            }
            Err(Problem::Fail(e)) => {
                broken_groups.insert(op.group);
                journal.finish_op(id, status::FAILED, Some(&e))?;
                report.failed.push((op.clone(), e));
            }
        }
    }

    report.removed_dirs = remove_emptied_dirs(vacated, &plan.cleanup_roots);

    let st = match (report.done, report.failed.len()) {
        (_, 0) => status::DONE,
        (0, _) => status::FAILED,
        _ => status::PARTIAL,
    };
    journal.finish_batch(batch, st)?;
    Ok(report)
}

/// 删除因整理而变空的文件夹，并逐级向上检查，但不越过 `roots`（也不删除 `roots` 本身）。
/// 只剩系统自动生成的文件（如 .DS_Store、Thumbs.db）的文件夹也算空。
fn remove_emptied_dirs(mut dirs: Vec<PathBuf>, roots: &[PathBuf]) -> Vec<PathBuf> {
    if roots.is_empty() {
        return Vec::new();
    }
    let root_keys: Vec<String> = roots
        .iter()
        .map(|r| path_key(r).trim_end_matches(['/', '\\']).to_string())
        .collect();
    // 严格位于某个范围之内
    let inside = |d: &Path| {
        let k = path_key(d);
        root_keys.iter().any(|r| {
            k.len() > r.len()
                && k.starts_with(r.as_str())
                && matches!(k.as_bytes()[r.len()], b'/' | b'\\')
        })
    };
    // 深的先处理，子文件夹删掉后父文件夹才可能变空
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    dirs.dedup();
    let mut removed = Vec::new();
    let mut done: std::collections::HashSet<String> = std::collections::HashSet::new();
    for start in dirs {
        let mut dir = start;
        while inside(&dir) && done.insert(path_key(&dir)) {
            if !remove_if_empty(&dir) {
                break;
            }
            removed.push(dir.clone());
            match dir.parent() {
                Some(p) => dir = p.to_path_buf(),
                None => break,
            }
        }
    }
    removed
}

fn remove_if_empty(dir: &Path) -> bool {
    let Ok(md) = dir.symlink_metadata() else {
        return false;
    };
    if !md.is_dir() {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    let mut junk = Vec::new();
    for e in entries {
        let Ok(e) = e else { return false };
        let name = e.file_name();
        let is_junk = name
            .to_str()
            .is_some_and(|n| crate::protect::SYSTEM_NAMES.contains(&n))
            && e.file_type().is_ok_and(|t| t.is_file());
        if !is_junk {
            return false;
        }
        junk.push(e.path());
    }
    for j in junk {
        if std::fs::remove_file(j).is_err() {
            return false;
        }
    }
    std::fs::remove_dir(dir).is_ok()
}

fn apply(op: &Op) -> Result<(), Problem> {
    let src = &op.src;
    if !exists(src) {
        return Err(Problem::Skip("源文件已不在原位置".into()));
    }
    let fail = |e: std::io::Error| Problem::Fail(e.to_string());
    match op.kind {
        OpKind::Trash => fsops::trash(src).map_err(Problem::Fail),
        OpKind::Move | OpKind::Rename => {
            let dst = dst_of(op)?;
            // 计划生成后目标位置可能又出现了文件，绝不覆盖。
            if exists(dst) && !is_case_only_change(src, dst) {
                return Err(Problem::Skip(
                    "目标位置出现了同名文件，为避免覆盖已跳过".into(),
                ));
            }
            fsops::move_file(src, dst).map_err(fail)
        }
        OpKind::Copy => {
            let dst = dst_of(op)?;
            if exists(dst) {
                return Err(Problem::Skip(
                    "目标位置出现了同名文件，为避免覆盖已跳过".into(),
                ));
            }
            fsops::copy_file(src, dst).map_err(fail)
        }
    }
}

fn dst_of(op: &Op) -> Result<&Path, Problem> {
    op.dst
        .as_deref()
        .ok_or_else(|| Problem::Fail("缺少目标路径".into()))
}
