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
