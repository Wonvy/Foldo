//! 文件类型分类表：只作为规则条件（`category: 图片`）和界面图标使用，
//! Foldo 不会按分类自动整理文件。

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Category {
    /// 稳定的英文标识，供界面选图标、做分组。
    pub id: &'static str,
    /// 默认的文件夹名。
    pub name: &'static str,
    /// 别名（写条件时也认，如 `category: Pictures`；比较时不区分大小写）。
    pub aliases: &'static [&'static str],
    pub extensions: &'static [&'static str],
}

pub static CATEGORIES: &[Category] = &[
    Category {
        id: "image",
        name: "图片",
        aliases: &["Pictures", "Images", "Image", "Photos", "照片", "图像"],
        extensions: &[
            "jpg", "jpeg", "png", "gif", "heic", "heif", "webp", "bmp", "tif", "tiff", "svg",
            "ico", "avif", "raw", "cr2", "cr3", "nef", "arw", "dng", "orf", "rw2",
        ],
    },
    Category {
        id: "document",
        name: "文档",
        aliases: &["Documents", "Docs", "Document"],
        extensions: &[
            "pdf", "doc", "docx", "txt", "md", "rtf", "odt", "pages", "wps", "epub", "mobi",
        ],
    },
    Category {
        id: "sheet",
        name: "表格与演示",
        aliases: &["Spreadsheets", "Presentations", "表格", "演示文稿"],
        extensions: &[
            "xls", "xlsx", "xlsm", "csv", "numbers", "ods", "et", "ppt", "pptx", "key", "odp",
            "dps",
        ],
    },
    Category {
        id: "archive",
        name: "压缩包",
        aliases: &["Archives", "Compressed", "压缩文件"],
        extensions: &["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "zst"],
    },
    Category {
        id: "installer",
        name: "安装包",
        aliases: &["Installers", "Setup", "安装程序"],
        extensions: &[
            "dmg", "pkg", "exe", "msi", "msix", "deb", "rpm", "appimage", "apk",
        ],
    },
    Category {
        id: "video",
        name: "视频",
        aliases: &["Videos", "Movies", "Video", "影片"],
        extensions: &[
            "mp4", "mov", "avi", "mkv", "wmv", "flv", "webm", "m4v", "mpg", "mpeg",
        ],
    },
    Category {
        id: "audio",
        name: "音频",
        aliases: &["Audio", "Music", "音乐"],
        extensions: &[
            "mp3", "wav", "flac", "aac", "m4a", "ogg", "wma", "aiff", "opus",
        ],
    },
    Category {
        id: "design",
        name: "设计文件",
        aliases: &["Design", "Designs", "设计"],
        extensions: &[
            "psd", "ai", "sketch", "fig", "xd", "indd", "cdr", "afdesign", "afphoto", "eps",
        ],
    },
    Category {
        id: "model",
        name: "3D 模型",
        aliases: &["3D", "Models", "CAD", "模型"],
        extensions: &[
            "step", "stp", "stl", "3mf", "obj", "fbx", "iges", "igs", "f3d", "sldprt", "sldasm",
            "slddrw", "blend", "glb", "gltf", "3ds", "dwg", "dxf", "x_t", "skp",
        ],
    },
    Category {
        id: "code",
        name: "代码",
        aliases: &["Code", "Source", "源码"],
        extensions: &[
            "py", "js", "ts", "jsx", "tsx", "rs", "go", "java", "kt", "c", "cpp", "h", "hpp", "cs",
            "swift", "rb", "php", "sh", "ps1", "bat", "json", "yaml", "yml", "toml", "xml", "html",
            "css", "sql", "ipynb",
        ],
    },
    Category {
        id: "font",
        name: "字体",
        aliases: &["Fonts"],
        extensions: &["ttf", "otf", "woff", "woff2", "ttc"],
    },
];

/// 按扩展名找分类（不区分大小写）。
pub fn by_extension(ext: &str) -> Option<&'static Category> {
    let ext = ext.to_lowercase();
    CATEGORIES
        .iter()
        .find(|c| c.extensions.iter().any(|e| *e == ext))
}

/// 每份配置独立的分类表，避免多个配置互相影响。
#[derive(Debug)]
pub struct CategoryMap(std::collections::HashMap<String, &'static Category>);

impl CategoryMap {
    pub fn compile(
        overrides: &std::collections::BTreeMap<String, Vec<String>>,
    ) -> Result<Self, String> {
        let mut map = std::collections::HashMap::new();
        for c in CATEGORIES {
            if !overrides.contains_key(c.id) {
                for ext in c.extensions {
                    map.insert((*ext).to_string(), c);
                }
            }
        }
        let mut custom = std::collections::HashMap::new();
        for (id, extensions) in overrides {
            let c = CATEGORIES
                .iter()
                .find(|c| c.id == id)
                .ok_or_else(|| format!("未知文件类型：{id}"))?;
            for raw in extensions {
                let ext = raw.trim().trim_start_matches('.').to_lowercase();
                if ext.is_empty()
                    || ext
                        .chars()
                        .any(|ch| ch.is_whitespace() || ".*/\\:?\"<>|".contains(ch))
                {
                    return Err(format!("无效扩展名：{raw}（填写最后一个点后的后缀）"));
                }
                if let Some(previous) = custom.insert(ext.clone(), c.id)
                    && previous != c.id
                {
                    return Err(format!("扩展名 {ext} 同时属于多个自定义类型"));
                }
                if let Some(previous) = map.insert(ext.clone(), c)
                    && previous.id != c.id
                {
                    return Err(format!(
                        "扩展名 {ext} 同时属于“{}”和“{}”，请只保留一处",
                        previous.name, c.name
                    ));
                }
            }
        }
        Ok(Self(map))
    }

    pub fn get(&self, ext: &str) -> Option<&'static Category> {
        self.0.get(&ext.to_lowercase()).copied()
    }
}

/// 按分类名、英文标识或同义词找分类（不区分大小写）。
pub fn find(name: &str) -> Option<&'static Category> {
    let n = name.trim().to_lowercase();
    CATEGORIES.iter().find(|c| {
        c.id == n || c.name.to_lowercase() == n || c.aliases.iter().any(|a| a.to_lowercase() == n)
    })
}

/// 列出所有分类名，用于报错提示。
pub fn names() -> String {
    CATEGORIES
        .iter()
        .map(|c| c.name)
        .collect::<Vec<_>>()
        .join("、")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup() {
        assert_eq!(by_extension("JPG").unwrap().name, "图片");
        assert_eq!(by_extension("step").unwrap().name, "3D 模型");
        assert!(by_extension("xyz").is_none());
        assert_eq!(find("Pictures").unwrap().id, "image");
        assert_eq!(find("图片").unwrap().id, "image");
        assert_eq!(find("model").unwrap().name, "3D 模型");
        assert!(find("不存在").is_none());
    }

    #[test]
    fn extensions_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for c in CATEGORIES {
            for e in c.extensions {
                assert!(seen.insert(*e), "扩展名 {e} 出现在多个分类中");
                assert_eq!(*e, e.to_lowercase());
            }
        }
    }
}
