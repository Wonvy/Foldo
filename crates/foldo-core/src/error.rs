use std::path::PathBuf;

/// 核心库错误。每个变体的 Display 都已包含完整信息（不挂 source 链），
/// 这样 CLI 用 `{:#}`、GUI 直接 `to_string()` 时都只显示一次。
/// 显示文字按当前语言翻译（见 [`crate::i18n`]）。
#[derive(Debug)]
pub enum Error {
    ReadConfig { path: PathBuf, err: std::io::Error },
    Parse(serde_yaml_ng::Error),
    InvalidRule { rule: String, msg: String },
    Journal(rusqlite::Error),
    Io { path: PathBuf, err: std::io::Error },
    Other(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Error::ReadConfig { path, err } => crate::t!(
                "读取规则文件失败 {path}: {err}",
                path = path.display(),
                err = err
            ),
            Error::Parse(e) => crate::t!("规则文件格式错误: {e}", e = e),
            Error::InvalidRule { rule, msg } => {
                crate::t!("规则「{rule}」无效: {msg}", rule = rule, msg = msg)
            }
            Error::Journal(e) => crate::t!("操作日志数据库错误: {e}", e = e),
            Error::Io { path, err } => format!("{}: {err}", path.display()),
            Error::Other(s) => s.clone(),
        };
        f.write_str(&s)
    }
}

impl std::error::Error for Error {}

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
