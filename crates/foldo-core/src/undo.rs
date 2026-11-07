//! 撤销一个批次：按日志逆序把文件放回原处。执行前逐项检查，绝不覆盖用户之后产生的文件。
//! `preview_undo` 只做检查不动文件，告诉用户哪些能恢复、哪些不能以及原因。

use crate::error::{Error, Result};
use crate::fsops::{self, exists};
use crate::journal::{Journal, KIND_MKDIR, OpRecord, status};
use crate::paths::{display, is_case_only_change, path_key};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct UndoReport {
    pub batch_id: i64,
    pub restored: usize,
    /// (路径, 原因)
    pub failed: Vec<(PathBuf, String)>,
    /// 移入回收站的文件无法自动恢复，需要用户到回收站手动还原。
    pub in_trash: Vec<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UndoOutcome {
    /// 可以恢复
    Restore,
    /// 在系统回收站中，需要手动还原
    InTrash,
    /// 无法恢复（文件被改动、移走或原位置被占用）
    Blocked,
}

#[derive(Debug, Clone, Serialize)]
pub struct UndoCheck {
    pub op_id: i64,
    pub kind: String,
    pub src: PathBuf,
    pub dst: Option<PathBuf>,
    pub outcome: UndoOutcome,
    pub reason: Option<String>,
}

fn ensure_undoable(journal: &Journal, batch_id: i64) -> Result<()> {
    let batch = journal
        .batch(batch_id)?
        .ok_or_else(|| Error::Other(format!("找不到批次 #{batch_id}")))?;
    if batch.status == status::UNDONE {
        return Err(Error::Other(format!("批次 #{batch_id} 已经撤销过了")));
    }
    Ok(())
}

/// 检查一个已完成的操作能否撤销。`exists` 由调用方提供，预览时用模拟的文件状态。
fn check(rec: &OpRecord, exists: &dyn Fn(&Path) -> bool) -> std::result::Result<(), String> {
    let dst = rec.dst.as_ref().ok_or("日志缺少目标路径")?;
    match rec.kind.as_str() {
        "move" | "rename" => {
            if !exists(dst) {
                return Err(format!("文件已不在 {}，可能被移走或删除", display(dst)));
            }
            if exists(&rec.src) && !is_case_only_change(&rec.src, dst) {
                return Err("原位置已经有同名的新文件，为避免覆盖不恢复".into());
            }
            Ok(())
        }
        "copy" => {
            if !exists(dst) {
                return Err(format!("副本已不在 {}", display(dst)));
            }
            // 预览时副本可能是模拟出来的，只在真实存在时比较大小
            if let (Some(expected), Ok(md)) = (rec.size, std::fs::metadata(dst))
                && md.len() != expected
            {
                return Err("副本之后被修改过，为安全起见保留".into());
            }
            Ok(())
        }
        other => Err(format!("未知的操作类型 {other}")),
    }
}

/// 撤销预览：逐项说明能否恢复。按真实撤销的顺序模拟文件状态，所以“改名 → 移动”这样的链条也能正确判断。
pub fn preview_undo(journal: &Journal, batch_id: i64) -> Result<Vec<UndoCheck>> {
    ensure_undoable(journal, batch_id)?;
    let ops = journal.ops(batch_id)?;
    // 模拟的文件状态：key → 是否存在；没有记录的路径看真实磁盘
    let mut overlay: HashMap<String, bool> = HashMap::new();
    let mut out = Vec::new();

    for rec in ops
        .iter()
        .rev()
        .filter(|r| r.status == status::DONE && r.kind != KIND_MKDIR)
    {
        let (outcome, reason) = if rec.kind == "trash" {
            (
                UndoOutcome::InTrash,
                Some("已在系统回收站中，需要手动还原".to_string()),
            )
        } else {
            let virt = |p: &Path| {
                overlay
                    .get(&path_key(p))
                    .copied()
                    .unwrap_or_else(|| exists(p))
            };
            match check(rec, &virt) {
                Ok(()) => {
                    if let Some(dst) = &rec.dst {
                        overlay.insert(path_key(dst), false);
                    }
                    if rec.kind != "copy" {
                        overlay.insert(path_key(&rec.src), true);
                    }
                    (UndoOutcome::Restore, None)
                }
                Err(e) => (UndoOutcome::Blocked, Some(e)),
            }
        };
        out.push(UndoCheck {
            op_id: rec.id,
            kind: rec.kind.clone(),
            src: rec.src.clone(),
            dst: rec.dst.clone(),
            outcome,
            reason,
        });
    }
    out.reverse();
    Ok(out)
}

pub fn undo_batch(journal: &Journal, batch_id: i64) -> Result<UndoReport> {
    ensure_undoable(journal, batch_id)?;
    let mut report = UndoReport {
        batch_id,
        ..Default::default()
    };
    let ops = journal.ops(batch_id)?;

    for rec in ops.iter().rev().filter(|r| r.status == status::DONE) {
        let result: std::result::Result<bool, String> = match rec.kind.as_str() {
            // 只删除仍为空的目录；非空说明用户放了别的东西进去，保留。
            KIND_MKDIR => Ok(std::fs::remove_dir(&rec.src).is_ok()),
            "trash" => {
                report.in_trash.push(rec.src.clone());
                continue;
            }
            _ => check(rec, &exists).and_then(|()| {
                let dst = rec.dst.as_ref().expect("check 已确认有目标路径");
                if rec.kind == "copy" {
                    std::fs::remove_file(dst).map_err(|e| e.to_string())?;
                } else {
                    if let Some(parent) = rec.src.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    fsops::move_file(dst, &rec.src).map_err(|e| e.to_string())?;
                }
                Ok(true)
            }),
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
