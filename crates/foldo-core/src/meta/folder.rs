//! 文件夹的属性：第一层内容（名称、扩展名、数量）、内容类型、文件夹类型、总大小。按需读取。

use crate::categories::{self, CategoryMap};
use chrono::{DateTime, Local};
use std::collections::HashMap;
use std::path::Path;

/// 文件夹类型的可选值。
pub const FOLDER_KINDS: &[&str] = &["代码工程", "应用程序", "空文件夹", "普通文件夹"];

/// 内容类型里“各种文件都有”的取值。
pub const MIXED: &str = "混合";

#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub modified: Option<DateTime<Local>>,
}

/// 文件夹第一层的内容（不含 .DS_Store、Thumbs.db 等系统文件）。
#[derive(Debug, Clone, Default)]
pub struct FolderMeta {
    pub entries: Vec<Entry>,
}

/// 说明是代码工程的标志性文件或文件夹。
const CODE_MARKERS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "package.json",
    "cargo.toml",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "pyproject.toml",
    "setup.py",
    "requirements.txt",
    "pipfile",
    "gemfile",
    "composer.json",
    "cmakelists.txt",
    "makefile",
    "package.swift",
    "pubspec.yaml",
    "deno.json",
    ".vscode",
    ".idea",
];
const CODE_EXTS: &[&str] = &[
    "sln",
    "csproj",
    "vcxproj",
    "xcodeproj",
    "xcworkspace",
    "uproject",
];

impl FolderMeta {
    pub fn read(dir: &Path) -> Option<Self> {
        let mut entries = Vec::new();
        for e in std::fs::read_dir(dir).ok()?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if crate::protect::SYSTEM_NAMES.contains(&name.as_str()) {
                continue;
            }
            let md = e.metadata().ok();
            entries.push(Entry {
                is_dir: md.as_ref().is_some_and(|m| m.is_dir()),
                modified: md.and_then(|m| m.modified().ok()).map(DateTime::from),
                name,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Some(Self { entries })
    }

    pub fn names(&self) -> Vec<String> {
        self.entries.iter().map(|e| e.name.clone()).collect()
    }

    pub fn file_count(&self) -> usize {
        self.entries.iter().filter(|e| !e.is_dir).count()
    }

    pub fn folder_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_dir).count()
    }

    /// 第一层文件的扩展名（小写，去重）。
    pub fn exts(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .entries
            .iter()
            .filter(|e| !e.is_dir)
            .filter_map(|e| {
                Path::new(&e.name)
                    .extension()
                    .map(|x| x.to_string_lossy().to_lowercase())
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// 第一层里最近一次改动的时间。
    pub fn last_change(&self) -> Option<DateTime<Local>> {
        self.entries.iter().filter_map(|e| e.modified).max()
    }

    /// 文件夹类型：代码工程 / 应用程序 / 空文件夹 / 普通文件夹。
    pub fn kind(&self, folder_name: &str) -> &'static str {
        let lower = folder_name.to_lowercase();
        if lower.ends_with(".app") {
            return "应用程序";
        }
        if self.entries.is_empty() {
            return "空文件夹";
        }
        let is_code = self.entries.iter().any(|e| {
            let n = e.name.to_lowercase();
            CODE_MARKERS.contains(&n.as_str())
                || Path::new(&n)
                    .extension()
                    .is_some_and(|x| CODE_EXTS.contains(&x.to_string_lossy().as_ref()))
        });
        if is_code {
            "代码工程"
        } else {
            "普通文件夹"
        }
    }

    /// 内容类型：第一层文件里超过一半是同一类型时为该类型（如“图片”），否则为“混合”。没有文件时为 None。
    pub fn content(&self, map: Option<&CategoryMap>) -> Option<String> {
        let mut counts: HashMap<&'static str, usize> = HashMap::new();
        let mut files = 0usize;
        for e in self.entries.iter().filter(|e| !e.is_dir) {
            files += 1;
            let ext = Path::new(&e.name)
                .extension()
                .map(|x| x.to_string_lossy().into_owned())
                .unwrap_or_default();
            let cat = match map {
                Some(m) => m.get(&ext),
                None => categories::by_extension(&ext),
            };
            if let Some(c) = cat {
                *counts.entry(c.name).or_default() += 1;
            }
        }
        if files == 0 {
            return None;
        }
        let top = counts
            .into_iter()
            .max_by_key(|(name, n)| (*n, std::cmp::Reverse(*name)));
        Some(match top {
            Some((name, n)) if n * 2 > files => name.to_string(),
            _ => MIXED.to_string(),
        })
    }
}

/// 文件夹总大小的缓存：后台每次检查都会重新扫描，大文件夹不必每次都遍历。
/// 文件夹本身有改动（第一层增删了东西）或超过 10 分钟就重新统计。
/// 文件夹 → (文件夹的修改时间, 统计的时刻, 总大小)
type SizeCache =
    HashMap<std::path::PathBuf, (Option<std::time::SystemTime>, std::time::Instant, u64)>;
static SIZES: std::sync::LazyLock<std::sync::Mutex<SizeCache>> =
    std::sync::LazyLock::new(Default::default);
const SIZE_TTL: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// 文件夹的总大小（含所有子文件夹；最多统计 20 万个项目，避免超大文件夹拖慢扫描）。
pub fn total_size(dir: &Path) -> u64 {
    let mtime = std::fs::metadata(dir).and_then(|m| m.modified()).ok();
    if let Ok(m) = SIZES.lock()
        && let Some((t, at, size)) = m.get(dir)
        && *t == mtime
        && at.elapsed() < SIZE_TTL
    {
        return *size;
    }
    let size = walk_size(dir);
    if let Ok(mut m) = SIZES.lock() {
        if m.len() > 10_000 {
            m.clear();
        }
        m.insert(dir.to_path_buf(), (mtime, std::time::Instant::now(), size));
    }
    size
}

fn walk_size(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .take(200_000)
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}
