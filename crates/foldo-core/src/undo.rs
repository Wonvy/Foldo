//! 撤销一个批次：按日志逆序把文件放回原处。执行前逐项检查，绝不覆盖用户之后产生的文件。

use crate::error::{Error, Result};
use crate::fsops::{self, exists};
use crate::journal::{Journal, KIND_MKDIR, status};
use crate::paths::{display, is_case_only_change};
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct UndoReport {
    pub batch_id: i64,
    pub restored: usize,
    /// (路径, 原因)
    pub failed: Vec<(PathBuf, String)>,
    /// 移入回收站的文件无法自动恢复，需要用户到回收站手动还原。
    pub in_trash: Vec<PathBuf>,
}

pub fn undo_batch(journal: &Journal, batch_id: i64) -> Result<UndoReport> {
    let batch = journal
        .batch(batch_id)?
        .ok_or_else(|| Error::Other(format!("找不到批次 #{batch_id}")))?;
    if batch.status == status::UNDONE {
        return Err(Error::Other(format!("批次 #{batch_id} 已经撤销过了")));
    }

    let mut report = UndoReport {
        batch_id,
        ..Default::default()
    };
    let ops = journal.ops(batch_id)?;

    for rec in ops.iter().rev().filter(|r| r.status == status::DONE) {
        let result: Result<bool, String> = match rec.kind.as_str() {
            "move" | "rename" => (|| {
                let dst = rec.dst.as_ref().ok_or("日志缺少目标路径")?;
                if !exists(dst) {
                    return Err(format!("文件已不在 {}", display(dst)));
                }
                if exists(&rec.src) && !is_case_only_change(&rec.src, dst) {
                    return Err("原位置已经有同名文件".into());
                }
                if let Some(parent) = rec.src.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                }
                fsops::move_file(dst, &rec.src).map_err(|e| e.to_string())?;
                Ok(true)
            })(),
            "copy" => (|| {
                let dst = rec.dst.as_ref().ok_or("日志缺少目标路径")?;
                let size = std::fs::metadata(dst).map(|m| m.len()).ok();
                if size.is_none() {
                    return Err(format!("副本已不在 {}", display(dst)));
                }
                if rec.size.is_some() && size != rec.size {
                    return Err("副本已被修改，为安全起见保留".into());
                }
                std::fs::remove_file(dst).map_err(|e| e.to_string())?;
                Ok(true)
            })(),
            // 只删除仍为空的目录；非空说明用户放了别的东西进去，保留。
            KIND_MKDIR => Ok(std::fs::remove_dir(&rec.src).is_ok()),
            "trash" => {
                report.in_trash.push(rec.src.clone());
                continue;
            }
            other => Err(format!("未知的操作类型 {other}")),
        };

        match result {
            Ok(true) => {
                journal.finish_op(rec.id, status::UNDONE, None)?;
                if rec.kind != KIND_MKDIR {
                    report.restored += 1;
                }
            }
            Ok(false) => {}
            Err(e) => report.failed.push((rec.src.clone(), e)),
        }
    }

    let st = if report.failed.is_empty() {
        status::UNDONE
    } else {
        status::PARTIALLY_UNDONE
    };
    journal.set_batch_status(batch_id, st)?;
    Ok(report)
}
