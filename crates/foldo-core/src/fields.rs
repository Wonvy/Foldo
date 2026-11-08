//! 字段注册表：规则能用到的全部文件属性。
//!
//! 条件、路径变量、图形界面的字段选择器和 `foldo inspect` 都从这一张表生成，保证处处一致。
//! 每个字段有固定的类型，类型决定能用哪些运算符。读不到的字段（如普通 PDF 的 `image.width`）
//! 视为“不存在”。

use crate::meta::FileCtx;
use chrono::{DateTime, Datelike, Local, Timelike};
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    Text,
    Number,
    /// 字节数，写值时可带单位（10MB）
    Size,
    Date,
    /// 有固定可选值的文本
    Enum,
    Bool,
    /// 一组名称（如文件夹里的项目名）：任意一项满足即算满足；“不包含 / 不等于”表示没有任何一项满足
    List,
}

/// 字段适用于文件、文件夹还是两者。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Applies {
    File,
    Folder,
    Both,
}

/// 字段的值。
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Text(String),
    Number(f64),
    Size(u64),
    Date(DateTime<Local>),
    Bool(bool),
    List(Vec<String>),
}

impl Value {
    /// 给人看的形式（也是路径变量的默认输出）。
    pub fn display(&self) -> String {
        match self {
            Value::Text(s) => s.clone(),
            Value::Number(n) => format_number(*n),
            Value::Size(b) => b.to_string(),
            Value::Date(d) => d.format("%Y-%m-%d %H:%M:%S").to_string(),
            Value::Bool(b) => if *b { "是" } else { "否" }.to_string(),
            Value::List(items) => items.join("、"),
        }
    }
}

pub fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        let s = format!("{n:.2}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

#[derive(Serialize)]
pub struct FieldDef {
    /// 在规则和变量里使用的名字，如 `image.width`。
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    #[serde(rename = "type")]
    pub ty: FieldType,
    /// 单位说明（像素、秒、页……）。
    pub unit: Option<&'static str>,
    pub desc: &'static str,
    /// 枚举字段的可选值。
    pub options: &'static [&'static str],
    /// 适用于文件、文件夹还是两者（规则编辑器按“处理对象”筛选字段）。
    pub applies: Applies,
    #[serde(skip)]
    pub get: fn(&FileCtx) -> Option<Value>,
}

impl std::fmt::Debug for FieldDef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.key)
    }
}

/// 与 categories::CATEGORIES 的名称一致（有测试保证）。
pub const CATEGORY_NAMES: &[&str] = &[
    "图片",
    "文档",
    "表格与演示",
    "压缩包",
    "安装包",
    "视频",
    "音频",
    "设计文件",
    "3D 模型",
    "代码",
    "字体",
];

pub const ORIENTATIONS: &[&str] = &["横", "竖", "方"];

/// 文件夹“内容类型”的可选值：文件类型名，加上“混合”。
pub const FOLDER_CONTENTS: &[&str] = &[
    "图片",
    "文档",
    "表格与演示",
    "压缩包",
    "安装包",
    "视频",
    "音频",
    "设计文件",
    "3D 模型",
    "代码",
    "字体",
    crate::meta::MIXED,
];

/// 文件和文件夹都能用的字段。
const BOTH: &[&str] = &[
    "name", "parent", "path", "size", "hidden", "readonly", "empty",
];

fn applies_of(f: &FieldDef) -> Applies {
    if f.group == "文件夹" {
        Applies::Folder
    } else if BOTH.contains(&f.key) || f.group == "时间" {
        Applies::Both
    } else {
        Applies::File
    }
}

fn text(s: impl Into<String>) -> Option<Value> {
    let s = s.into();
    (!s.is_empty()).then_some(Value::Text(s))
}

fn date_part(d: Option<DateTime<Local>>, part: fn(&DateTime<Local>) -> u32) -> Option<Value> {
    d.map(|d| Value::Number(part(&d) as f64))
}

fn year(d: &DateTime<Local>) -> u32 {
    d.year() as u32
}
fn month(d: &DateTime<Local>) -> u32 {
    d.month()
}
fn day(d: &DateTime<Local>) -> u32 {
    d.day()
}
/// 周一为 1，周日为 7。
fn weekday(d: &DateTime<Local>) -> u32 {
    d.weekday().number_from_monday()
}
fn hour(d: &DateTime<Local>) -> u32 {
    d.hour()
}

macro_rules! field {
    ($key:expr, $label:expr, $group:literal, $ty:ident, $unit:expr, $desc:literal, $get:expr) => {
        field!($key, $label, $group, $ty, $unit, $desc, &[], $get)
    };
    ($key:expr, $label:expr, $group:literal, $ty:ident, $unit:expr, $desc:literal, $opts:expr, $get:expr) => {
        FieldDef {
            key: $key,
            label: $label,
            group: $group,
            ty: FieldType::$ty,
            unit: $unit,
            desc: $desc,
            options: $opts,
            applies: Applies::File,
            get: $get,
        }
    };
}

macro_rules! date_parts {
    ($base:literal, $label:literal, $src:ident) => {
        [
            field!(
                concat!($base, ".year"),
                concat!($label, "·年"),
                "时间",
                Number,
                None,
                "四位年份，如 2026",
                |f| date_part(f.$src, year)
            ),
            field!(
                concat!($base, ".month"),
                concat!($label, "·月"),
                "时间",
                Number,
                None,
                "1–12",
                |f| date_part(f.$src, month)
            ),
            field!(
                concat!($base, ".day"),
                concat!($label, "·日"),
                "时间",
                Number,
                None,
                "1–31",
                |f| date_part(f.$src, day)
            ),
            field!(
                concat!($base, ".weekday"),
                concat!($label, "·星期"),
                "时间",
                Number,
                None,
                "周一为 1，周日为 7",
                |f| date_part(f.$src, weekday)
            ),
            field!(
                concat!($base, ".hour"),
                concat!($label, "·小时"),
                "时间",
                Number,
                None,
                "0–23",
                |f| date_part(f.$src, hour)
            ),
        ]
    };
}

fn media_text(
    f: &FileCtx,
    pick: fn(&crate::meta::MediaMeta) -> Option<&String>,
    video: bool,
) -> Option<Value> {
    f.media()
        .filter(|m| m.is_video == video)
        .and_then(pick)
        .and_then(|s| text(s.clone()))
}

pub static FIELDS: std::sync::LazyLock<Vec<FieldDef>> = std::sync::LazyLock::new(|| {
    let mut v = vec![
        // ---------- 基本 ----------
        field!(
            "name",
            "文件名",
            "基本",
            Text,
            None,
            "含扩展名，如 报告.pdf",
            |f| text(f.name.clone())
        ),
        field!(
            "stem",
            "主名",
            "基本",
            Text,
            None,
            "不含扩展名，如 报告",
            |f| text(f.stem.clone())
        ),
        field!(
            "ext",
            "扩展名",
            "基本",
            Text,
            None,
            "不含点，如 pdf",
            |f| text(f.ext.clone())
        ),
        field!(
            "parent",
            "所在文件夹",
            "基本",
            Text,
            None,
            "上一级文件夹的名字",
            |f| {
                f.path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| text(n.to_string_lossy()))
            }
        ),
        field!(
            "path",
            "完整路径",
            "基本",
            Text,
            None,
            "文件的完整路径",
            |f| text(f.path.to_string_lossy())
        ),
        field!(
            "size",
            "大小",
            "基本",
            Size,
            Some("B / KB / MB / GB"),
            "文件大小；文件夹为里面所有文件的总大小",
            |f| Some(Value::Size(f.total_size()))
        ),
        field!(
            "category",
            "类型",
            "基本",
            Enum,
            None,
            "按扩展名归类",
            CATEGORY_NAMES,
            |f| { f.category().map(|c| Value::Text(c.name.to_string())) }
        ),
        field!(
            "mime",
            "真实格式",
            "基本",
            Text,
            None,
            "按文件头识别，如 image/png；改了扩展名也认得出",
            |f| { f.mime().and_then(text) }
        ),
        field!(
            "hidden",
            "隐藏",
            "基本",
            Bool,
            None,
            "是否为隐藏文件",
            |f| Some(Value::Bool(f.hidden))
        ),
        field!(
            "readonly",
            "只读",
            "基本",
            Bool,
            None,
            "是否为只读文件",
            |f| Some(Value::Bool(f.readonly))
        ),
        field!(
            "empty",
            "空的",
            "基本",
            Bool,
            None,
            "文件大小为 0，或文件夹里什么都没有",
            |f| Some(Value::Bool(match f.folder() {
                Some(m) => m.entries.is_empty(),
                None => !f.is_dir && f.size == 0,
            }))
        ),
        field!(
            "duplicate",
            "重复文件",
            "基本",
            Bool,
            None,
            "同一个整理文件夹里有内容完全相同的文件；最早修改的一份算原件，不算重复",
            |f| f.is_duplicate().map(Value::Bool)
        ),
        field!(
            "duplicate.of",
            "重复的原件",
            "基本",
            Text,
            None,
            "重复文件对应的原件文件名",
            |f| f
                .duplicate_original()
                .and_then(|p| p.file_name())
                .map(|n| Value::Text(n.to_string_lossy().into_owned()))
        ),
        // ---------- 文件夹 ----------
        field!(
            "folder.kind",
            "文件夹类型",
            "文件夹",
            Enum,
            None,
            "代码工程（有 .git、package.json 等）、应用程序（.app）、空文件夹或普通文件夹",
            crate::meta::FOLDER_KINDS,
            |f| f.folder().map(|m| Value::Text(m.kind(&f.name).to_string()))
        ),
        field!(
            "folder.content",
            "内容类型",
            "文件夹",
            Enum,
            None,
            "第一层文件里超过一半是同一类型时为该类型（如 图片），否则为 混合",
            FOLDER_CONTENTS,
            |f| f.folder_content().map(Value::Text)
        ),
        field!(
            "folder.names",
            "里面的项目名",
            "文件夹",
            List,
            None,
            "第一层的文件和文件夹名；任意一个满足即可，如“包含 README”“符合 *.sln”",
            |f| f.folder().map(|m| Value::List(m.names()))
        ),
        field!(
            "folder.exts",
            "里面的扩展名",
            "文件夹",
            List,
            None,
            "第一层文件的扩展名（小写），如 有 jpg",
            |f| f.folder().map(|m| Value::List(m.exts()))
        ),
        field!(
            "folder.items",
            "项目数",
            "文件夹",
            Number,
            Some("个"),
            "第一层的文件和文件夹总数",
            |f| f.folder().map(|m| Value::Number(m.entries.len() as f64))
        ),
        field!(
            "folder.files",
            "文件数",
            "文件夹",
            Number,
            Some("个"),
            "第一层的文件数",
            |f| f.folder().map(|m| Value::Number(m.file_count() as f64))
        ),
        field!(
            "folder.folders",
            "子文件夹数",
            "文件夹",
            Number,
            Some("个"),
            "第一层的子文件夹数",
            |f| f.folder().map(|m| Value::Number(m.folder_count() as f64))
        ),
        field!(
            "folder.changed",
            "里面最近改动",
            "文件夹",
            Date,
            None,
            "第一层里最近一次修改的时间，如“超过 30 天没动过”",
            |f| f
                .folder()
                .and_then(|m| m.last_change().or(f.modified))
                .map(Value::Date)
        ),
        // ---------- 时间 ----------
        field!(
            "created",
            "创建时间",
            "时间",
            Date,
            None,
            "文件创建时间（部分 Linux 文件系统没有）",
            |f| f.created.map(Value::Date)
        ),
        field!(
            "modified",
            "修改时间",
            "时间",
            Date,
            None,
            "最后一次修改内容的时间",
            |f| f.modified.map(Value::Date)
        ),
        field!(
            "accessed",
            "访问时间",
            "时间",
            Date,
            None,
            "最后一次打开的时间（系统可能不实时更新）",
            |f| f.accessed.map(Value::Date)
        ),
    ];
    // 日期分量紧跟在时间字段后面
    v.extend(date_parts!("created", "创建", created));
    v.extend(date_parts!("modified", "修改", modified));
    v.extend(date_parts!("accessed", "访问", accessed));
    v.extend([
        // ---------- 图片 ----------
        field!(
            "image.width",
            "图片宽度",
            "图片",
            Number,
            Some("像素"),
            "只读文件头，速度快",
            |f| { f.image_size().map(|(w, _)| Value::Number(w as f64)) }
        ),
        field!(
            "image.height",
            "图片高度",
            "图片",
            Number,
            Some("像素"),
            "只读文件头，速度快",
            |f| { f.image_size().map(|(_, h)| Value::Number(h as f64)) }
        ),
        field!(
            "image.pixels",
            "总像素",
            "图片",
            Number,
            Some("百万像素"),
            "宽 × 高 ÷ 1,000,000",
            |f| {
                f.image_size().map(|(w, h)| {
                    Value::Number(((w as f64 * h as f64) / 1e6 * 100.0).round() / 100.0)
                })
            }
        ),
        field!(
            "image.ratio",
            "宽高比",
            "图片",
            Number,
            None,
            "宽 ÷ 高，如 16:9 约为 1.78",
            |f| {
                f.image_size()
                    .filter(|(_, h)| *h > 0)
                    .map(|(w, h)| Value::Number(((w as f64 / h as f64) * 100.0).round() / 100.0))
            }
        ),
        field!(
            "image.orientation",
            "方向",
            "图片",
            Enum,
            None,
            "横 / 竖 / 方",
            ORIENTATIONS,
            |f| {
                f.image_size().map(|(w, h)| {
                    Value::Text(
                        match w.cmp(&h) {
                            std::cmp::Ordering::Greater => "横",
                            std::cmp::Ordering::Less => "竖",
                            std::cmp::Ordering::Equal => "方",
                        }
                        .into(),
                    )
                })
            }
        ),
        // ---------- 照片 EXIF ----------
        field!(
            "exif.taken",
            "拍摄时间",
            "照片 EXIF",
            Date,
            None,
            "相机记录的拍摄时间",
            |f| {
                f.media()
                    .filter(|m| !m.is_video)
                    .and_then(|m| m.taken)
                    .map(Value::Date)
            }
        ),
        field!(
            "exif.make",
            "相机品牌",
            "照片 EXIF",
            Text,
            None,
            "如 Apple、Canon",
            |f| media_text(f, |m| m.make.as_ref(), false)
        ),
        field!(
            "exif.model",
            "相机型号",
            "照片 EXIF",
            Text,
            None,
            "如 iPhone 16 Pro",
            |f| media_text(f, |m| m.model.as_ref(), false)
        ),
        field!(
            "exif.lens",
            "镜头",
            "照片 EXIF",
            Text,
            None,
            "镜头型号",
            |f| media_text(f, |m| m.lens.as_ref(), false)
        ),
        field!(
            "exif.software",
            "处理软件",
            "照片 EXIF",
            Text,
            None,
            "如 Adobe Lightroom",
            |f| media_text(f, |m| m.software.as_ref(), false)
        ),
        field!(
            "exif.iso",
            "ISO",
            "照片 EXIF",
            Number,
            None,
            "感光度",
            |f| {
                f.media()
                    .filter(|m| !m.is_video)
                    .and_then(|m| m.iso)
                    .map(Value::Number)
            }
        ),
        field!(
            "exif.focal",
            "焦距",
            "照片 EXIF",
            Number,
            Some("毫米"),
            "拍摄焦距",
            |f| {
                f.media()
                    .filter(|m| !m.is_video)
                    .and_then(|m| m.focal)
                    .map(Value::Number)
            }
        ),
        field!(
            "exif.gps",
            "有定位",
            "照片 EXIF",
            Bool,
            None,
            "照片是否带 GPS 坐标",
            |f| {
                f.media()
                    .filter(|m| !m.is_video)
                    .map(|m| Value::Bool(m.gps))
            }
        ),
        // ---------- 视频 ----------
        field!(
            "video.duration",
            "视频时长",
            "视频",
            Number,
            Some("秒"),
            "可写 90、5m、1h",
            |f| {
                f.media()
                    .filter(|m| m.is_video)
                    .and_then(|m| m.duration)
                    .map(Value::Number)
            }
        ),
        field!(
            "video.width",
            "视频宽度",
            "视频",
            Number,
            Some("像素"),
            "画面宽度",
            |f| {
                f.media()
                    .filter(|m| m.is_video)
                    .and_then(|m| m.width)
                    .map(|w| Value::Number(w as f64))
            }
        ),
        field!(
            "video.height",
            "视频高度",
            "视频",
            Number,
            Some("像素"),
            "画面高度",
            |f| {
                f.media()
                    .filter(|m| m.is_video)
                    .and_then(|m| m.height)
                    .map(|h| Value::Number(h as f64))
            }
        ),
        field!(
            "video.taken",
            "拍摄时间",
            "视频",
            Date,
            None,
            "视频文件记录的创建时间",
            |f| {
                f.media()
                    .filter(|m| m.is_video)
                    .and_then(|m| m.taken)
                    .map(Value::Date)
            }
        ),
        field!(
            "video.model",
            "拍摄设备",
            "视频",
            Text,
            None,
            "如 iPhone 16 Pro",
            |f| media_text(f, |m| m.model.as_ref(), true)
        ),
        // ---------- 音频 ----------
        field!(
            "audio.duration",
            "音频时长",
            "音频",
            Number,
            Some("秒"),
            "可写 90、5m、1h",
            |f| { f.audio().and_then(|a| a.duration).map(Value::Number) }
        ),
        field!(
            "audio.bitrate",
            "码率",
            "音频",
            Number,
            Some("kbps"),
            "如 320",
            |f| {
                f.audio()
                    .and_then(|a| a.bitrate)
                    .map(|b| Value::Number(b as f64))
            }
        ),
        field!(
            "audio.title",
            "歌曲名",
            "音频",
            Text,
            None,
            "标签里的标题",
            |f| f.audio().and_then(|a| a.title.clone()).and_then(text)
        ),
        field!("audio.artist", "艺术家", "音频", Text, None, "", |f| f
            .audio()
            .and_then(|a| a.artist.clone())
            .and_then(text)),
        field!("audio.album", "专辑", "音频", Text, None, "", |f| f
            .audio()
            .and_then(|a| a.album.clone())
            .and_then(text)),
        field!("audio.genre", "流派", "音频", Text, None, "", |f| f
            .audio()
            .and_then(|a| a.genre.clone())
            .and_then(text)),
        field!(
            "audio.year",
            "年份",
            "音频",
            Number,
            None,
            "发行年份",
            |f| {
                f.audio()
                    .and_then(|a| a.year)
                    .map(|y| Value::Number(y as f64))
            }
        ),
        // ---------- PDF ----------
        field!("pdf.pages", "页数", "PDF", Number, Some("页"), "", |f| f
            .pdf()
            .and_then(|p| p.pages)
            .map(|n| Value::Number(n as f64))),
        field!(
            "pdf.title",
            "标题",
            "PDF",
            Text,
            None,
            "文档属性里的标题",
            |f| f.pdf().and_then(|p| p.title.clone()).and_then(text)
        ),
        field!("pdf.author", "作者", "PDF", Text, None, "", |f| f
            .pdf()
            .and_then(|p| p.author.clone())
            .and_then(text)),
        // ---------- Office 文档 ----------
        field!(
            "doc.title",
            "标题",
            "Office 文档",
            Text,
            None,
            "docx / xlsx / pptx 的文档属性",
            |f| { f.office().and_then(|d| d.title.clone()).and_then(text) }
        ),
        field!(
            "doc.author",
            "作者",
            "Office 文档",
            Text,
            None,
            "",
            |f| f.office().and_then(|d| d.author.clone()).and_then(text)
        ),
        field!(
            "doc.company",
            "公司",
            "Office 文档",
            Text,
            None,
            "",
            |f| f.office().and_then(|d| d.company.clone()).and_then(text)
        ),
        field!(
            "doc.pages",
            "页数",
            "Office 文档",
            Number,
            Some("页"),
            "Word 文档",
            |f| {
                f.office()
                    .and_then(|d| d.pages)
                    .map(|n| Value::Number(n as f64))
            }
        ),
        field!(
            "doc.slides",
            "幻灯片数",
            "Office 文档",
            Number,
            Some("页"),
            "PowerPoint 演示文稿",
            |f| {
                f.office()
                    .and_then(|d| d.slides)
                    .map(|n| Value::Number(n as f64))
            }
        ),
        // ---------- 内容 ----------
        field!(
            "content",
            "文本内容",
            "内容",
            Text,
            None,
            "纯文本类文件（txt、md、csv、json……）的前 1 MB",
            |f| { f.content().and_then(|c| text(c.to_string())) }
        ),
        // ---------- 下载来源 ----------
        field!(
            "source.url",
            "下载网址",
            "下载来源",
            Text,
            None,
            "从浏览器下载时记录的网址",
            |f| { f.source_url().and_then(|u| text(u.to_string())) }
        ),
        field!(
            "source.domain",
            "来源域名",
            "下载来源",
            Text,
            None,
            "如 mail.qq.com",
            |f| { f.source_url().and_then(crate::meta::domain).and_then(text) }
        ),
    ]);
    for f in &mut v {
        f.applies = applies_of(f);
    }
    v
});

/// 按名字找字段。兼容旧写法 `extension`。
pub fn field(key: &str) -> Option<&'static FieldDef> {
    let key = match key {
        "extension" => "ext",
        k => k,
    };
    FIELDS.iter().find(|f| f.key == key)
}

/// 一个文件的全部属性（用于“查看文件属性”和 `foldo inspect`）。
pub fn inspect(file: &FileCtx) -> Vec<(&'static FieldDef, Option<Value>)> {
    FIELDS.iter().map(|f| (f, (f.get)(file))).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::categories::CATEGORIES;

    #[test]
    fn category_names_match_table() {
        let names: Vec<&str> = CATEGORIES.iter().map(|c| c.name).collect();
        assert_eq!(names, CATEGORY_NAMES);
    }

    #[test]
    fn keys_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for f in FIELDS.iter() {
            assert!(seen.insert(f.key), "字段重复：{}", f.key);
        }
        assert!(field("extension").is_some());
        assert!(field("modified.weekday").is_some());
    }
}
