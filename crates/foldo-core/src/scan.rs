//! 扫描一个位置。每个项目要么成为候选文件，要么带着原因被跳过——不会被悄悄丢掉。

use crate::fileinfo::{FileInfo, is_hidden};
use crate::meta::FileCtx;
use crate::paths::display;
use crate::protect::{Protect, SkipKind, builtin_exclusion, is_partial_or_system_dir};
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Default)]
pub struct ScanResult {
    pub files: Vec<FileCtx>,
    pub skipped: Vec<(PathBuf, SkipKind, String)>,
    pub warnings: Vec<String>,
}

/// 判断单个文件是否需要跳过（扫描目录和处理拖入的文件共用）。
pub fn classify_file(
    path: &Path,
    name: &str,
    md: &Metadata,
    include_hidden: bool,
    protect: &Protect,
) -> Option<(SkipKind, String)> {
    if let Some(reason) = protect.check(path, name) {
        return Some((SkipKind::Protected, reason));
    }
    if let Some(hit) = builtin_exclusion(path, name) {
        return Some(hit);
    }
    if !include_hidden && is_hidden(name, md) {
        return Some((SkipKind::Hidden, "隐藏文件".into()));
    }
    None
}

/// 扫描 `root`。不递归时，第一层的文件夹作为整体保留；递归时进入子文件夹，
/// 但固定保留、隐藏、系统和下载中的文件夹不进入。
pub fn scan(root: &Path, recursive: bool, include_hidden: bool, protect: &Protect) -> ScanResult {
    let mut res = ScanResult::default();
    let mut it = WalkDir::new(root)
        .min_depth(1)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();

    while let Some(entry) = it.next() {
        let entry = match entry {
            Ok(e) => e,
            Err(err) => {
                let at = err.path().map(display).unwrap_or_default();
                res.warnings.push(format!("无法读取 {at}: {err}"));
                continue;
            }
        };
        let path = entry.path();
        if path.to_str().is_none() {
            res.warnings.push(format!(
                "跳过文件名不是 UTF-8 编码的项目: {}",
                path.display()
            ));
            if entry.file_type().is_dir() {
                it.skip_current_dir();
            }
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let md = match entry.metadata() {
            Ok(m) => m,
            Err(err) => {
                res.warnings
                    .push(format!("无法读取 {}: {err}", display(path)));
                continue;
            }
        };

        if entry.file_type().is_dir() {
            let skip = if let Some(reason) = protect.check(path, &name) {
                Some((SkipKind::Protected, reason))
            } else if let Some(kind) = is_partial_or_system_dir(&name) {
                Some((kind, kind.label().to_string()))
            } else if !include_hidden && is_hidden(&name, &md) {
                Some((SkipKind::Hidden, "隐藏文件夹".into()))
            } else if !recursive {
                Some((
                    SkipKind::Folder,
                    "已有文件夹作为整体保留，不拆分内部文件".into(),
                ))
            } else {
                None
            };
            if let Some((kind, reason)) = skip {
                res.skipped.push((entry.into_path(), kind, reason));
                if recursive {
                    it.skip_current_dir();
                }
            }
            continue;
        }

        // 不跟随符号链接，只处理普通文件。
        if !entry.file_type().is_file() {
            continue;
        }
        match classify_file(path, &name, &md, include_hidden, protect) {
            Some((kind, reason)) => res.skipped.push((entry.into_path(), kind, reason)),
            None => {
                let hidden = is_hidden(&name, &md);
                res.files
                    .push(FileInfo::new(entry.into_path(), &md, hidden).into());
            }
        }
    }
    res
}
