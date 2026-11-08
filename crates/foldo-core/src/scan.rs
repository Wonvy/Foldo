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
    /// 第一层的子文件夹（固定保留、隐藏、系统和下载中的除外），供文件夹规则判断。
    /// 没有文件夹规则处理的，由调用方记为“作为整体保留”。
    pub folders: Vec<FileCtx>,
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
        return Some((SkipKind::Hidden, crate::t!("隐藏文件")));
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
                res.warnings
                    .push(crate::t!("无法读取 {at}: {err}", at = at, err = err));
                continue;
            }
        };
        let path = entry.path();
        if path.to_str().is_none() {
            res.warnings.push(crate::t!(
                "跳过文件名不是 UTF-8 编码的项目: {a0}",
                a0 = path.display()
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
                res.warnings.push(crate::t!(
                    "无法读取 {a0}: {err}",
                    a0 = display(path),
                    err = err
                ));
                continue;
            }
        };

        if entry.file_type().is_dir() {
            let skip = if let Some(reason) = protect.check(path, &name) {
                Some((SkipKind::Protected, reason))
            } else if let Some(kind) = is_partial_or_system_dir(&name) {
                Some((kind, crate::i18n::tr(kind.label())))
            } else if !include_hidden && is_hidden(&name, &md) {
                Some((SkipKind::Hidden, crate::t!("隐藏文件夹")))
            } else {
                None
            };
            if let Some((kind, reason)) = skip {
                res.skipped.push((entry.into_path(), kind, reason));
                if recursive {
                    it.skip_current_dir();
                }
                continue;
            }
            // 应用程序包（macOS 的 .app）看起来是文件夹，但不能拆开
            let bundle = name.to_lowercase().ends_with(".app");
            if entry.depth() == 1 {
                let hidden = is_hidden(&name, &md);
                res.folders
                    .push(FileInfo::new(path.to_path_buf(), &md, hidden).into());
            }
            if recursive && bundle {
                it.skip_current_dir();
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
    attach_peers(&mut res.files);
    res
}

/// 让同一次扫描的文件互相知道彼此，用于判断重复文件。
pub fn attach_peers(files: &mut [FileCtx]) {
    let peers = std::rc::Rc::new(crate::meta::Peers::new(
        files.iter().map(|f| (f.path.as_path(), f.size, f.modified)),
    ));
    for f in files {
        f.set_peers(peers.clone());
    }
}
