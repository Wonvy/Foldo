//! 按计划执行操作，并逐条写入操作日志。单个操作出问题不会中断整个批次。
//!
//! 结果分三类：成功；跳过（执行时才出现的新冲突，如目标位置刚出现同名文件、源文件已被移走）；
//! 失败（权限不足、磁盘错误等）。同一个文件的动作链中有一步没完成，后面的步骤一律跳过。

use crate::error::Result;
use crate::fsops::{self, exists};
use crate::journal::{Journal, KIND_MKDIR, status};
use crate::paths::is_case_only_change;
use crate::plan::{Op, OpKind, Plan};
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Default)]
pub struct ExecReport {
    pub batch_id: i64,
    pub done: usize,
    pub skipped: Vec<(Op, String)>,
    pub failed: Vec<(Op, String)>,
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

    let st = match (report.done, report.failed.len()) {
        (_, 0) => status::DONE,
        (0, _) => status::FAILED,
        _ => status::PARTIAL,
    };
    journal.finish_batch(batch, st)?;
    Ok(report)
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
