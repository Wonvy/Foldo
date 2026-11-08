//! 文件上下文：基本信息直接可用；图片尺寸、EXIF、真实格式等元数据按需读取并缓存。
//!
//! 只有规则里实际用到的字段才会触发读取。例如没有任何规则用到 `exif.*`，就完全不会解析 EXIF。
//! 一个 `FileCtx` 在一次扫描中被所有规则共享，所以每类元数据最多读一次。

mod docs;
mod dup;
mod media;

use crate::categories;
use crate::fileinfo::FileInfo;
use std::cell::{Cell, OnceCell};
use std::ops::Deref;
use std::path::PathBuf;
use std::rc::Rc;

pub use docs::{AudioMeta, OfficeMeta, PdfMeta, domain};
pub use dup::Peers;
pub use media::MediaMeta;

#[derive(Debug)]
pub struct FileCtx {
    info: FileInfo,
    image_size: OnceCell<Option<(u32, u32)>>,
    media: OnceCell<Option<MediaMeta>>,
    mime: OnceCell<Option<String>>,
    audio: OnceCell<Option<AudioMeta>>,
    pdf: OnceCell<Option<PdfMeta>>,
    office: OnceCell<Option<OfficeMeta>>,
    source: OnceCell<Option<String>>,
    content: OnceCell<Option<String>>,
    /// 同一次扫描里的其他文件（判断重复用）；单独查看一个文件时没有
    peers: Option<Rc<Peers>>,
    duplicate_of: OnceCell<Option<PathBuf>>,
    /// 实际读取元数据的次数（测试按需读取用）。
    reads: Cell<u32>,
}

impl Deref for FileCtx {
    type Target = FileInfo;
    fn deref(&self) -> &FileInfo {
        &self.info
    }
}

impl From<FileInfo> for FileCtx {
    fn from(info: FileInfo) -> Self {
        Self {
            info,
            image_size: OnceCell::new(),
            media: OnceCell::new(),
            mime: OnceCell::new(),
            audio: OnceCell::new(),
            pdf: OnceCell::new(),
            office: OnceCell::new(),
            source: OnceCell::new(),
            content: OnceCell::new(),
            peers: None,
            duplicate_of: OnceCell::new(),
            reads: Cell::new(0),
        }
    }
}

impl FileCtx {
    pub fn from_path(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        Ok(FileInfo::from_path(&path.into())?.into())
    }

    pub fn info(&self) -> &FileInfo {
        &self.info
    }

    /// 按扩展名判断的内置类型（只是判断依据，不会据此自动整理）。
    pub fn category(&self) -> Option<&'static categories::Category> {
        categories::by_extension(&self.ext)
    }

    fn is_category(&self, id: &str) -> bool {
        self.category().is_some_and(|c| c.id == id)
    }

    /// 设置同一次扫描里的全部文件，之后才能判断是否重复。
    pub fn set_peers(&mut self, peers: Rc<Peers>) {
        self.peers = Some(peers);
    }

    /// 是否是重复文件：None 表示无法判断（单独查看一个文件时）。
    pub fn is_duplicate(&self) -> Option<bool> {
        self.peers.as_ref()?;
        Some(self.duplicate_original().is_some())
    }

    /// 这个文件是重复的副本时，返回原件的路径。
    pub fn duplicate_original(&self) -> Option<&PathBuf> {
        self.duplicate_of
            .get_or_init(|| {
                let peers = self.peers.as_ref()?;
                self.count_read();
                peers.original_of(&self.path, self.size)
            })
            .as_ref()
    }

    /// 已经读取过多少次元数据。
    pub fn metadata_reads(&self) -> u32 {
        self.reads.get()
    }

    fn count_read(&self) {
        self.reads.set(self.reads.get() + 1);
    }

    /// 图片像素尺寸（只读文件头）。
    pub fn image_size(&self) -> Option<(u32, u32)> {
        *self.image_size.get_or_init(|| {
            if !self.is_category("image") {
                return None;
            }
            self.count_read();
            imagesize::size(&self.path)
                .ok()
                .map(|s| (s.width as u32, s.height as u32))
        })
    }

    /// 照片 EXIF 或视频轨道信息。
    pub fn media(&self) -> Option<&MediaMeta> {
        self.media
            .get_or_init(|| {
                if !(self.is_category("image") || self.is_category("video")) {
                    return None;
                }
                self.count_read();
                media::read(&self.path)
            })
            .as_ref()
    }

    /// 按文件头判断的真实格式（MIME），与扩展名无关。纯文本等没有特征头的文件为空。
    pub fn mime(&self) -> Option<&str> {
        self.mime
            .get_or_init(|| {
                if self.size == 0 {
                    return None;
                }
                self.count_read();
                infer::get_from_path(&self.path)
                    .ok()
                    .flatten()
                    .map(|t| t.mime_type().to_string())
            })
            .as_deref()
    }

    fn ext_is(&self, list: &[&str]) -> bool {
        let e = self.ext.to_lowercase();
        list.contains(&e.as_str())
    }

    /// 音频时长、码率和标签。
    pub fn audio(&self) -> Option<&AudioMeta> {
        self.audio
            .get_or_init(|| {
                if !self.is_category("audio") {
                    return None;
                }
                self.count_read();
                docs::audio(&self.path)
            })
            .as_ref()
    }

    /// PDF 页数、标题、作者（只读元数据）。
    pub fn pdf(&self) -> Option<&PdfMeta> {
        self.pdf
            .get_or_init(|| {
                if !self.ext_is(&["pdf"]) {
                    return None;
                }
                self.count_read();
                docs::pdf(&self.path)
            })
            .as_ref()
    }

    /// Office 文档（docx / xlsx / pptx）的标题、作者、公司、页数、幻灯片数。
    pub fn office(&self) -> Option<&OfficeMeta> {
        self.office
            .get_or_init(|| {
                if !self.ext_is(&["docx", "docm", "xlsx", "xlsm", "pptx", "pptm"]) {
                    return None;
                }
                self.count_read();
                docs::office(&self.path)
            })
            .as_ref()
    }

    /// 下载来源网址。
    pub fn source_url(&self) -> Option<&str> {
        self.source
            .get_or_init(|| {
                self.count_read();
                docs::source_url(&self.path)
            })
            .as_deref()
    }

    /// 纯文本文件的内容（最多前 1 MB）。
    pub fn content(&self) -> Option<&str> {
        self.content
            .get_or_init(|| {
                if self.size == 0 {
                    return None;
                }
                self.count_read();
                docs::content(&self.path, &self.ext)
            })
            .as_deref()
    }
}
