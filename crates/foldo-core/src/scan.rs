//! 扫描一个位置下的文件，跳过隐藏文件、未下载完的文件和系统杂项文件。

use crate::fileinfo::{FileInfo, is_hidden};
use crate::paths::display;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// 下载中的临时文件后缀（Chrome、Firefox、Safari、Edge、Opera 等）。
const PARTIAL_SUFFIXES: &[&str] = &[
    ".crdownload",
    ".part",
    ".partial",
    ".download",
    ".opdownload",
];

/// 系统自动生成的杂项文件。
const JUNK_NAMES: &[&str] = &[".DS_Store", ".localized", "Thumbs.db", "desktop.ini"];

/// 是否一定要跳过（不管规则怎么写）。
pub fn is_ignored_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    JUNK_NAMES.iter().any(|j| j.eq_ignore_ascii_case(name))
        || PARTIAL_SUFFIXES.iter().any(|s| lower.ends_with(s))
        // Office 打开文档时生成的锁文件
        || name.starts_with("~$")
}

pub struct ScanResult {
    pub files: Vec<FileInfo>,
    pub warnings: Vec<String>,
}

pub fn scan(root: &Path, recursive: bool, include_hidden: bool) -> ScanResult {
    let mut files = Vec::new();
    let mut warnings = Vec::new();

    let keep_dir = |e: &DirEntry| -> bool {
        if e.depth() == 0 || !e.file_type().is_dir() {
            return true;
        }
        let name = e.file_name().to_string_lossy();
        if is_ignored_name(&name) {
            return false;
        }
        if include_hidden {
            return true;
        }
        let hidden = e
            .metadata()
            .map(|m| is_hidden(&name, &m))
            .unwrap_or_else(|_| name.starts_with('.'));
        !hidden
    };

    let walker = WalkDir::new(root)
        .min_depth(1)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .follow_links(false)
        .sort_by_file_name();

    for entry in walker.into_iter().filter_entry(keep_dir) {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                let at = err.path().map(display).unwrap_or_default();
                warnings.push(format!("无法读取 {at}: {err}"));
                continue;
            }
        };
        // 不跟随符号链接，只处理普通文件。
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().to_str().is_none() {
            warnings.push(format!(
                "跳过文件名不是 UTF-8 编码的文件: {}",
                entry.path().display()
            ));
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if is_ignored_name(&name) {
            continue;
        }
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                warnings.push(format!("无法读取 {}: {err}", display(entry.path())));
                continue;
            }
        };
        let hidden = is_hidden(&name, &md);
        if hidden && !include_hidden {
            continue;
        }
        files.push(FileInfo::new(entry.into_path(), &md, hidden));
    }

    ScanResult { files, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_names() {
        assert!(is_ignored_name(".DS_Store"));
        assert!(is_ignored_name("movie.mp4.crdownload"));
        assert!(is_ignored_name("setup.dmg.download"));
        assert!(is_ignored_name("~$report.docx"));
        assert!(!is_ignored_name("report.docx"));
    }
}
