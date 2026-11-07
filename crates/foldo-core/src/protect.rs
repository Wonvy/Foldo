//! 保护与排除：哪些东西永远不动，以及“为什么没整理它”的原因分类。

use crate::paths::{expand_home, path_key};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Serialize;
use std::io::Read;
use std::path::{MAIN_SEPARATOR, Path, PathBuf};

/// 某个项目没有进入整理计划的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SkipKind {
    /// 用户设置的固定保留
    Protected,
    /// 私钥、证书、密码库等凭证文件
    Credential,
    /// 快捷方式
    Shortcut,
    /// 系统自动生成的文件
    System,
    /// 隐藏文件
    Hidden,
    /// 下载未完成
    Partial,
    /// 处于新文件缓冲期
    Buffer,
    /// 正在被其他程序使用
    InUse,
    /// 文件夹作为整体保留
    Folder,
    /// 没有匹配的规则
    NoMatch,
    /// 目标位置冲突且规则要求跳过
    Conflict,
    /// 规则无法应用到这个文件（如路径模板缺少信息）
    Invalid,
}

impl SkipKind {
    pub fn label(self) -> &'static str {
        match self {
            SkipKind::Protected => "固定保留",
            SkipKind::Credential => "凭证文件",
            SkipKind::Shortcut => "快捷方式",
            SkipKind::System => "系统文件",
            SkipKind::Hidden => "隐藏文件",
            SkipKind::Partial => "下载未完成",
            SkipKind::Buffer => "新文件缓冲期",
            SkipKind::InUse => "正在使用",
            SkipKind::Folder => "文件夹",
            SkipKind::NoMatch => "没有匹配的规则",
            SkipKind::Conflict => "目标冲突",
            SkipKind::Invalid => "无法应用规则",
        }
    }
}

/// 系统自动生成的文件。
const SYSTEM_NAMES: &[&str] = &[
    ".DS_Store",
    ".localized",
    "Icon\r",
    "Thumbs.db",
    "ehthumbs.db",
    "desktop.ini",
    "$RECYCLE.BIN",
    "System Volume Information",
];

/// 下载中的临时文件后缀（Chrome、Firefox、Safari、Edge、Opera 等）。
const PARTIAL_SUFFIXES: &[&str] = &[
    ".crdownload",
    ".part",
    ".partial",
    ".download",
    ".opdownload",
];

const SHORTCUT_EXTS: &[&str] = &["lnk", "url", "webloc", "desktop", "inetloc"];

const CREDENTIAL_EXTS: &[&str] = &[
    "pem", "p12", "pfx", "keystore", "jks", "kdbx", "kdb", "ppk", "ovpn", "gpg", "pgp",
];

const CREDENTIAL_PREFIXES: &[&str] = &["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519", ".env"];

/// 不看用户设置、按文件名和内容就能判断的排除项。
pub fn builtin_exclusion(path: &Path, name: &str) -> Option<(SkipKind, String)> {
    if SYSTEM_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Some((SkipKind::System, "系统自动生成的文件".into()));
    }
    let lower = name.to_lowercase();
    if PARTIAL_SUFFIXES.iter().any(|s| lower.ends_with(s)) || name.starts_with("~$") {
        return Some((
            SkipKind::Partial,
            "正在下载或被 Office 打开的临时文件".into(),
        ));
    }
    let ext = Path::new(&lower)
        .extension()
        .map(|e| e.to_string_lossy().into_owned())
        .unwrap_or_default();
    if CREDENTIAL_EXTS.contains(&ext.as_str())
        || CREDENTIAL_PREFIXES.iter().any(|p| lower.starts_with(p))
        || (lower.contains("credentials") && matches!(ext.as_str(), "json" | "csv" | "txt"))
        || (ext == "key" && looks_like_pem(path))
    {
        return Some((
            SkipKind::Credential,
            "可能是私钥、证书或密码库，默认不移动".into(),
        ));
    }
    if SHORTCUT_EXTS.contains(&ext.as_str()) {
        return Some((SkipKind::Shortcut, "快捷方式默认保留在原位".into()));
    }
    None
}

/// `.key` 既可能是私钥，也可能是 Keynote 演示文稿；看文件开头区分。
fn looks_like_pem(path: &Path) -> bool {
    let mut buf = [0u8; 64];
    let Ok(mut f) = std::fs::File::open(path) else {
        return false;
    };
    let n = f.read(&mut buf).unwrap_or(0);
    String::from_utf8_lossy(&buf[..n]).contains("-----BEGIN")
}

/// 文件或文件夹名本身就该跳过（不进入、不列出内容）。
pub fn is_partial_or_system_dir(name: &str) -> Option<SkipKind> {
    let lower = name.to_lowercase();
    if SYSTEM_NAMES.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        Some(SkipKind::System)
    } else if PARTIAL_SUFFIXES.iter().any(|s| lower.ends_with(s)) {
        Some(SkipKind::Partial)
    } else {
        None
    }
}

/// 用户设置的固定保留列表。
#[derive(Debug, Default)]
pub struct Protect {
    paths: Vec<(PathBuf, String)>,
    globs: GlobSet,
    patterns: Vec<String>,
}

impl Protect {
    /// 条目含路径分隔符或以 `~` 开头视为路径（保护它本身及其下所有内容），否则视为文件名通配符。
    pub fn compile(entries: &[String]) -> Result<Self, String> {
        let mut paths = Vec::new();
        let mut builder = GlobSetBuilder::new();
        let mut patterns = Vec::new();
        for raw in entries {
            let e = raw.trim();
            if e.is_empty() {
                continue;
            }
            if e.starts_with('~') || e.contains('/') || e.contains('\\') {
                let p = expand_home(e);
                if !p.is_absolute() {
                    return Err(format!("固定保留的路径「{e}」必须是绝对路径或以 ~ 开头"));
                }
                paths.push((p.clone(), path_key(&p)));
            } else {
                let glob = GlobBuilder::new(e)
                    .case_insensitive(true)
                    .build()
                    .map_err(|err| format!("固定保留的通配符「{e}」无效: {err}"))?;
                builder.add(glob);
                patterns.push(e.to_string());
            }
        }
        Ok(Self {
            paths,
            globs: builder.build().map_err(|e| e.to_string())?,
            patterns,
        })
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty() && self.patterns.is_empty()
    }

    /// 是否受保护；受保护时返回说明。
    pub fn check(&self, path: &Path, name: &str) -> Option<String> {
        let key = path_key(path);
        for (p, pk) in &self.paths {
            if key == *pk || key.starts_with(&format!("{pk}{MAIN_SEPARATOR}")) {
                return Some(format!("在固定保留列表中（{}）", crate::paths::display(p)));
            }
        }
        if self.globs.is_match(name) {
            return Some("文件名符合固定保留的通配符".into());
        }
        None
    }
}

/// 文件是否正被其他程序占用（只在 Windows 上能可靠判断：以独占方式打开会失败）。
#[cfg(windows)]
pub fn is_in_use(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;
    match std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(path)
    {
        Ok(_) => false,
        Err(e) => matches!(
            e.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION)
        ),
    }
}

/// 文件是否正被其他程序占用（macOS / Linux 没有强制锁，无法可靠判断，一律视为未占用）。
#[cfg(not(windows))]
pub fn is_in_use(_path: &Path) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin() {
        let p = Path::new("/nonexistent");
        let kind = |n: &str| builtin_exclusion(p, n).map(|(k, _)| k);
        assert_eq!(kind(".DS_Store"), Some(SkipKind::System));
        assert_eq!(kind("big.iso.crdownload"), Some(SkipKind::Partial));
        assert_eq!(kind("~$报告.docx"), Some(SkipKind::Partial));
        assert_eq!(kind("server.pem"), Some(SkipKind::Credential));
        assert_eq!(kind("id_ed25519"), Some(SkipKind::Credential));
        assert_eq!(kind("aws_credentials.csv"), Some(SkipKind::Credential));
        assert_eq!(kind("Chrome.lnk"), Some(SkipKind::Shortcut));
        // 打不开的 .key 不视为私钥（Keynote 演示文稿也是 .key）
        assert_eq!(kind("演示.key"), None);
        assert_eq!(kind("报告.pdf"), None);
    }

    #[test]
    fn protect_paths_and_globs() {
        let home = dirs::home_dir().unwrap();
        let pr = Protect::compile(&["~/Downloads/项目A".into(), "*.lic".into()]).unwrap();
        assert!(pr.check(&home.join("Downloads/项目A"), "项目A").is_some());
        assert!(
            pr.check(&home.join("Downloads/项目A/图.png"), "图.png")
                .is_some()
        );
        assert!(
            pr.check(&home.join("Downloads/项目AB.png"), "项目AB.png")
                .is_none()
        );
        assert!(pr.check(&home.join("Downloads/x.LIC"), "x.LIC").is_some());
        assert!(Protect::compile(&["relative/dir".into()]).is_err());
    }
}
