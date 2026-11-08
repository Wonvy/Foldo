//! 操作日志（SQLite）。每次执行是一个批次，每个操作执行前先写入 pending，完成后更新状态，
//! 这样即使中途崩溃，也能知道哪些操作已经完成、可以撤销。

use crate::error::{Error, Result};
use chrono::Local;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub mod status {
    pub const RUNNING: &str = "running";
    pub const PENDING: &str = "pending";
    pub const DONE: &str = "done";
    pub const FAILED: &str = "failed";
    /// 执行时遇到新冲突而跳过的单个操作
    pub const SKIPPED: &str = "skipped";
    pub const PARTIAL: &str = "partial";
    pub const UNDONE: &str = "undone";
    pub const PARTIALLY_UNDONE: &str = "partially_undone";
}

/// 日志里额外记录的操作类型：执行时新建的目录。
pub const KIND_MKDIR: &str = "mkdir";

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS batches (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    started_at  TEXT NOT NULL,
    finished_at TEXT,
    source      TEXT NOT NULL,
    status      TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS ops (
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    batch_id INTEGER NOT NULL REFERENCES batches(id),
    seq      INTEGER NOT NULL,
    rule     TEXT,
    kind     TEXT NOT NULL,
    src      TEXT NOT NULL,
    dst      TEXT,
    size     INTEGER,
    status   TEXT NOT NULL,
    error    TEXT
);
CREATE INDEX IF NOT EXISTS ops_batch ON ops(batch_id, seq);
"#;

/// 一段时间内的整理统计。
#[derive(Debug, Clone, Default, Serialize, PartialEq, Eq)]
pub struct Stats {
    /// 整理过的文件数（改名再移动这样的一串操作算一个）
    pub files: u64,
    /// 移到回收站的文件数和总大小（腾出的空间）
    pub trashed: u64,
    pub trashed_bytes: u64,
    /// 自动整理（后台监听、定时）和手动整理的次数
    pub auto_runs: u64,
    pub manual_runs: u64,
    /// 用得最多的规则和次数
    pub top_rules: Vec<(String, u64)>,
}

pub struct Journal {
    conn: Connection,
}

#[derive(Debug, Clone, Serialize)]
pub struct BatchSummary {
    pub id: i64,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub source: String,
    pub status: String,
    /// 不含 mkdir 的操作数。
    pub op_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OpRecord {
    pub id: i64,
    pub seq: i64,
    pub rule: Option<String>,
    pub kind: String,
    pub src: PathBuf,
    pub dst: Option<PathBuf>,
    pub size: Option<u64>,
    pub status: String,
    pub error: Option<String>,
}

fn now() -> String {
    Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

fn path_str(p: &Path) -> String {
    // 扫描阶段已经过滤掉了非 UTF-8 路径
    p.to_string_lossy().into_owned()
}

impl Journal {
    /// 打开（必要时创建）数据目录下的 `journal.db`。
    pub fn open_in(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir).map_err(|err| Error::Io {
            path: data_dir.to_path_buf(),
            err,
        })?;
        Self::init(Connection::open(data_dir.join("journal.db"))?)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self { conn })
    }

    /// 删除开始时间早于 `days` 天前的批次及其操作记录，返回删除的批次数。`days` 为 0 时不清理。
    pub fn prune(&self, days: u32) -> Result<usize> {
        if days == 0 {
            return Ok(0);
        }
        let cutoff = (Local::now() - chrono::Duration::days(days as i64))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "DELETE FROM ops WHERE batch_id IN (SELECT id FROM batches WHERE started_at < ?1)",
            params![cutoff],
        )?;
        let n = tx.execute("DELETE FROM batches WHERE started_at < ?1", params![cutoff])?;
        tx.commit()?;
        Ok(n)
    }

    /// 从 `since`（本地时间，`%Y-%m-%d %H:%M:%S`）起的整理统计。撤销了的操作不算。
    pub fn stats(&self, since: &str) -> Result<Stats> {
        let one = |sql: &str| -> Result<u64> {
            Ok(self
                .conn
                .query_row(sql, params![since, status::DONE, KIND_MKDIR], |r| {
                    r.get::<_, i64>(0)
                })
                .map(|n| n.max(0) as u64)?)
        };
        // 同一个文件的一串操作（改名再移动）只算一个文件
        let files = one(
            "SELECT COUNT(*) FROM (SELECT DISTINCT o.batch_id, o.src FROM ops o JOIN batches b ON b.id = o.batch_id
             WHERE b.started_at >= ?1 AND o.status = ?2 AND o.kind != ?3
             AND NOT EXISTS (SELECT 1 FROM ops p WHERE p.batch_id = o.batch_id AND p.seq < o.seq
                             AND p.dst = o.src AND p.status = ?2))",
        )?;
        let trashed_bytes = one(
            "SELECT COALESCE(SUM(o.size), 0) FROM ops o JOIN batches b ON b.id = o.batch_id
             WHERE b.started_at >= ?1 AND o.status = ?2 AND o.kind = 'trash' AND o.kind != ?3",
        )?;
        let trashed = one(
            "SELECT COUNT(*) FROM ops o JOIN batches b ON b.id = o.batch_id
             WHERE b.started_at >= ?1 AND o.status = ?2 AND o.kind = 'trash' AND o.kind != ?3",
        )?;
        let runs = |source: &str| -> Result<u64> {
            Ok(self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM batches WHERE started_at >= ?1 AND source = ?2",
                    params![since, source],
                    |r| r.get::<_, i64>(0),
                )
                .map(|n| n.max(0) as u64)?)
        };
        let mut stmt = self.conn.prepare(
            "SELECT o.rule, COUNT(*) AS n FROM ops o JOIN batches b ON b.id = o.batch_id
             WHERE b.started_at >= ?1 AND o.status = ?2 AND o.kind != ?3 AND o.rule IS NOT NULL AND o.rule != ''
             GROUP BY o.rule ORDER BY n DESC, o.rule LIMIT 3",
        )?;
        let top_rules = stmt
            .query_map(params![since, status::DONE, KIND_MKDIR], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?.max(0) as u64))
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(Stats {
            files,
            trashed,
            trashed_bytes,
            auto_runs: runs("watch")?,
            manual_runs: runs("gui")? + runs("cli")?,
            top_rules,
        })
    }

    pub fn begin_batch(&self, source: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO batches (started_at, source, status) VALUES (?1, ?2, ?3)",
            params![now(), source, status::RUNNING],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_batch(&self, id: i64, st: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE batches SET finished_at = ?1, status = ?2 WHERE id = ?3",
            params![now(), st, id],
        )?;
        Ok(())
    }

    pub fn set_batch_status(&self, id: i64, st: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE batches SET status = ?1 WHERE id = ?2",
            params![st, id],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn begin_op(
        &self,
        batch: i64,
        seq: i64,
        rule: &str,
        kind: &str,
        src: &Path,
        dst: Option<&Path>,
        size: Option<u64>,
    ) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO ops (batch_id, seq, rule, kind, src, dst, size, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                batch,
                seq,
                rule,
                kind,
                path_str(src),
                dst.map(path_str),
                size.map(|s| s as i64),
                status::PENDING
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn finish_op(&self, op_id: i64, st: &str, error: Option<&str>) -> Result<()> {
        self.conn.execute(
            "UPDATE ops SET status = ?1, error = ?2 WHERE id = ?3",
            params![st, error, op_id],
        )?;
        Ok(())
    }

    pub fn batch(&self, id: i64) -> Result<Option<BatchSummary>> {
        Ok(self
            .conn
            .query_row(&format!("{BATCH_SELECT} WHERE b.id = ?1"), [id], batch_row)
            .optional()?)
    }

    /// 最近的批次，新的在前。
    pub fn batches(&self, limit: usize) -> Result<Vec<BatchSummary>> {
        let mut stmt = self
            .conn
            .prepare(&format!("{BATCH_SELECT} ORDER BY b.id DESC LIMIT ?1"))?;
        let rows = stmt.query_map([limit as i64], batch_row)?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }

    /// 最近一个还可以撤销的批次。
    pub fn last_undoable(&self) -> Result<Option<i64>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id FROM batches WHERE status != ?1 ORDER BY id DESC LIMIT 1",
                [status::UNDONE],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn ops(&self, batch: i64) -> Result<Vec<OpRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, seq, rule, kind, src, dst, size, status, error
             FROM ops WHERE batch_id = ?1 ORDER BY seq",
        )?;
        let rows = stmt.query_map([batch], |r| {
            Ok(OpRecord {
                id: r.get(0)?,
                seq: r.get(1)?,
                rule: r.get(2)?,
                kind: r.get(3)?,
                src: PathBuf::from(r.get::<_, String>(4)?),
                dst: r.get::<_, Option<String>>(5)?.map(PathBuf::from),
                size: r.get::<_, Option<i64>>(6)?.map(|s| s as u64),
                status: r.get(7)?,
                error: r.get(8)?,
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    }
}

const BATCH_SELECT: &str = "SELECT b.id, b.started_at, b.finished_at, b.source, b.status,
    (SELECT COUNT(*) FROM ops o WHERE o.batch_id = b.id AND o.kind != 'mkdir')
    FROM batches b";

fn batch_row(r: &rusqlite::Row) -> rusqlite::Result<BatchSummary> {
    Ok(BatchSummary {
        id: r.get(0)?,
        started_at: r.get(1)?,
        finished_at: r.get(2)?,
        source: r.get(3)?,
        status: r.get(4)?,
        op_count: r.get(5)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prune_removes_only_old_batches() {
        let j = Journal::open_in_memory().unwrap();
        let old = j.begin_batch("gui").unwrap();
        let op = j
            .begin_op(
                old,
                1,
                "r",
                "move",
                Path::new("/a"),
                Some(Path::new("/b")),
                Some(1),
            )
            .unwrap();
        j.finish_op(op, status::DONE, None).unwrap();
        j.finish_batch(old, status::DONE).unwrap();
        // 把第一批改成 400 天前
        let long_ago = (Local::now() - chrono::Duration::days(400))
            .format("%Y-%m-%d %H:%M:%S")
            .to_string();
        j.conn
            .execute(
                "UPDATE batches SET started_at = ?1 WHERE id = ?2",
                params![long_ago, old],
            )
            .unwrap();
        let recent = j.begin_batch("gui").unwrap();
        j.finish_batch(recent, status::DONE).unwrap();

        // 0 表示一直保留
        assert_eq!(j.prune(0).unwrap(), 0);
        assert_eq!(j.prune(365).unwrap(), 1);
        assert!(j.batch(old).unwrap().is_none());
        assert!(j.ops(old).unwrap().is_empty());
        assert!(j.batch(recent).unwrap().is_some());
    }

    #[test]
    fn stats_count_files_space_and_rules() {
        let j = Journal::open_in_memory().unwrap();
        let b = j.begin_batch("gui").unwrap();
        let mut seq = 0;
        let mut op = |rule: &str, kind: &str, src: &str, dst: Option<&str>, size: u64, st: &str| {
            seq += 1;
            let id = j
                .begin_op(
                    b,
                    seq,
                    rule,
                    kind,
                    Path::new(src),
                    dst.map(Path::new),
                    Some(size),
                )
                .unwrap();
            j.finish_op(id, st, None).unwrap();
        };
        // 同一个文件先改名再移动：算一个文件
        op("截图", "rename", "/a.png", Some("/b.png"), 10, status::DONE);
        op("截图", "move", "/b.png", Some("/s/b.png"), 10, status::DONE);
        op("清理", "trash", "/old.zip", None, 1000, status::DONE);
        // 复制再压缩：原文件都留在原处，也只算一个文件
        op(
            "备份",
            "copy",
            "/e.txt",
            Some("/bak/e.txt"),
            3,
            status::DONE,
        );
        op(
            "备份",
            "compress",
            "/e.txt",
            Some("/e.zip"),
            3,
            status::DONE,
        );
        op(
            "文档",
            "move",
            "/c.pdf",
            Some("/d/c.pdf"),
            5,
            status::FAILED,
        );
        j.finish_batch(b, status::PARTIAL).unwrap();
        let w = j.begin_batch("watch").unwrap();
        j.finish_batch(w, status::DONE).unwrap();

        let s = j.stats("2000-01-01 00:00:00").unwrap();
        assert_eq!(s.trashed, 1);
        assert_eq!(s.trashed_bytes, 1000);
        assert_eq!(s.manual_runs, 1);
        assert_eq!(s.auto_runs, 1);
        // 截图和备份各 2 次，并列时按名称排
        assert_eq!(
            s.top_rules[..2],
            [("备份".to_string(), 2), ("截图".to_string(), 2)]
        );
        // a.png（改名再移动）、old.zip、e.txt（复制再压缩）
        assert_eq!(s.files, 3, "{s:?}");
        // 时间范围之外的不算
        assert_eq!(j.stats("2999-01-01 00:00:00").unwrap(), Stats::default());
    }
}
