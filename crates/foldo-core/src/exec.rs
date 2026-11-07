//! 按计划执行操作，并逐条写入操作日志。单个操作失败不会中断整个批次。

use crate::error::Result;
use crate::fsops::{self, exists};
use crate::journal::{Journal, KIND_MKDIR, status};
use crate::paths::is_case_only_change;
use crate::plan::{Op, OpKind, Plan};
use std::path::Path;

#[derive(Debug)]
pub struct ExecReport {
    pub batch_id: i64,
    pub done: usize,
    pub failed: Vec<(Op, String)>,
}

/// 执行计划。`source` 记录这次执行从哪里触发（如 `cli`、`gui`、`watch`）。
pub fn execute(plan: &Plan, journal: &Journal, source: &str) -> Result<ExecReport> {
    let batch = journal.begin_batch(source)?;
    let mut report = ExecReport {
        batch_id: batch,
        done: 0,
        failed: Vec::new(),
    };
    let mut seq: i64 = 0;

    for op in &plan.ops {
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
                    report
                        .failed
                        .push((op.clone(), format!("无法创建目标文件夹: {e}")));
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
            Err(e) => {
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

fn apply(op: &Op) -> Result<(), String> {
    let src = &op.src;
    if !exists(src) {
        return Err("源文件已不存在".into());
    }
    match op.kind {
        OpKind::Trash => fsops::trash(src),
        OpKind::Move | OpKind::Rename => {
            let dst = dst_of(op)?;
            // 计划生成后目标位置可能又出现了文件，绝不覆盖。
            if exists(dst) && !is_case_only_change(src, dst) {
                return Err("目标位置已有文件，为避免覆盖已跳过".into());
            }
            fsops::move_file(src, dst).map_err(|e| e.to_string())
        }
        OpKind::Copy => {
            let dst = dst_of(op)?;
            if exists(dst) {
                return Err("目标位置已有文件，为避免覆盖已跳过".into());
            }
            fsops::copy_file(src, dst).map_err(|e| e.to_string())
        }
    }
}

fn dst_of(op: &Op) -> Result<&Path, String> {
    op.dst.as_deref().ok_or_else(|| "缺少目标路径".to_string())
}
