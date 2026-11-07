use std::path::PathBuf;

/// 核心库错误。每个变体的 Display 都已包含完整信息（不挂 source 链），
/// 这样 CLI 用 `{:#}`、GUI 直接 `to_string()` 时都只显示一次。
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("读取规则文件失败 {path}: {err}")]
    ReadConfig { path: PathBuf, err: std::io::Error },

    #[error("规则文件格式错误: {0}")]
    Parse(serde_yaml_ng::Error),

    #[error("规则「{rule}」无效: {msg}")]
    InvalidRule { rule: String, msg: String },

    #[error("操作日志数据库错误: {0}")]
    Journal(rusqlite::Error),

    #[error("{path}: {err}")]
    Io { path: PathBuf, err: std::io::Error },

    #[error("{0}")]
    Other(String),
}

impl From<serde_yaml_ng::Error> for Error {
    fn from(e: serde_yaml_ng::Error) -> Self {
        Error::Parse(e)
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Journal(e)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
