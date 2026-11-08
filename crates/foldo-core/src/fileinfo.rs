use chrono::{DateTime, Local};
use std::fs::Metadata;
use std::path::{Path, PathBuf};

/// 扫描得到的文件快照。
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub path: PathBuf,
    pub name: String,
    /// 不含扩展名的文件名（`a.tar.gz` → `a.tar`）。
    pub stem: String,
    /// 扩展名，不含点，保留原始大小写（`a.JPG` → `JPG`）。
    pub ext: String,
    pub size: u64,
    pub created: Option<DateTime<Local>>,
    pub modified: Option<DateTime<Local>>,
    pub accessed: Option<DateTime<Local>>,
    pub hidden: bool,
    pub readonly: bool,
    /// 是文件夹（文件夹规则处理的对象）。
    pub is_dir: bool,
}

impl FileInfo {
    pub fn new(path: PathBuf, md: &Metadata, hidden: bool) -> Self {
        let is_dir = md.is_dir();
        let (name, mut stem, mut ext) = split_name(&path);
        if is_dir {
            // 文件夹名里的点不当作扩展名（“项目.v2” 的主名就是 “项目.v2”）
            stem = name.clone();
            ext = String::new();
        }
        Self {
            is_dir,
            name,
            stem,
            ext,
            size: md.len(),
            created: md.created().ok().map(DateTime::from),
            modified: md.modified().ok().map(DateTime::from),
            accessed: md.accessed().ok().map(DateTime::from),
            hidden,
            readonly: md.permissions().readonly(),
            path,
        }
    }

    pub fn from_path(path: &Path) -> std::io::Result<Self> {
        let md = std::fs::symlink_metadata(path)?;
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default();
        let hidden = is_hidden(&name, &md);
        Ok(Self::new(path.to_path_buf(), &md, hidden))
    }
}

/// 拆出 (文件名, 主名, 扩展名)。
pub fn split_name(path: &Path) -> (String, String, String) {
    let s = |o: Option<&std::ffi::OsStr>| {
        o.map(|v| v.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    (
        s(path.file_name()),
        s(path.file_stem()),
        s(path.extension()),
    )
}

/// 是否为隐藏文件：以点开头；Windows 上带隐藏属性；macOS 上带 UF_HIDDEN 标记。
pub fn is_hidden(name: &str, md: &Metadata) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if md.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
            return true;
        }
    }
    #[cfg(target_os = "macos")]
    {
        use std::os::macos::fs::MetadataExt;
        const UF_HIDDEN: u32 = 0x8000;
        if md.st_flags() & UF_HIDDEN != 0 {
            return true;
        }
    }
    let _ = md;
    false
}
