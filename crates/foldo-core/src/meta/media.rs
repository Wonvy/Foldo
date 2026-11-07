//! 照片 EXIF 和视频轨道信息（nom-exif，纯 Rust 实现，不依赖 FFmpeg）。

use chrono::{DateTime, Local, TimeZone};
use nom_exif::{EntryValue, ExifDateTime, ExifTag, Metadata, TrackInfoTag};
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct MediaMeta {
    /// 拍摄时间（照片的 DateTimeOriginal，视频的创建时间）。
    pub taken: Option<DateTime<Local>>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub lens: Option<String>,
    pub software: Option<String>,
    pub iso: Option<f64>,
    pub focal: Option<f64>,
    pub gps: bool,
    /// 视频时长（秒）。
    pub duration: Option<f64>,
    /// 视频画面宽高。
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// 是否来自视频轨道（否则来自照片 EXIF）。
    pub is_video: bool,
}

fn text(v: Option<&EntryValue>) -> Option<String> {
    v.and_then(|v| v.as_str())
        .map(|s| s.trim_matches(char::from(0)).trim().to_string())
        .filter(|s| !s.is_empty())
}

fn number(v: Option<&EntryValue>) -> Option<f64> {
    let v = v?;
    v.try_as_float().or_else(|| {
        // ISO 有时以数组形式存储
        v.as_u16_slice().and_then(|a| a.first()).map(|x| *x as f64)
    })
}

fn local(v: Option<&EntryValue>) -> Option<DateTime<Local>> {
    match v?.as_datetime()? {
        ExifDateTime::Aware(dt) => Some(dt.with_timezone(&Local)),
        // 没有时区信息的拍摄时间按本地时间理解（相机就是这么记录的）
        ExifDateTime::Naive(n) => Local.from_local_datetime(&n).earliest(),
    }
}

pub fn read(path: &Path) -> Option<MediaMeta> {
    match nom_exif::read_metadata(path).ok()? {
        Metadata::Exif(exif) => Some(MediaMeta {
            taken: local(exif.get(ExifTag::DateTimeOriginal))
                .or_else(|| local(exif.get(ExifTag::CreateDate)))
                .or_else(|| local(exif.get(ExifTag::ModifyDate))),
            make: text(exif.get(ExifTag::Make)),
            model: text(exif.get(ExifTag::Model)),
            lens: text(exif.get(ExifTag::LensModel)),
            software: text(exif.get(ExifTag::Software)),
            iso: number(exif.get(ExifTag::ISOSpeedRatings)),
            focal: number(exif.get(ExifTag::FocalLength)),
            gps: exif.gps_info().is_some(),
            duration: None,
            width: None,
            height: None,
            is_video: false,
        }),
        Metadata::Track(t) => Some(MediaMeta {
            taken: local(t.get(TrackInfoTag::CreateDate)),
            make: text(t.get(TrackInfoTag::Make)),
            model: text(t.get(TrackInfoTag::Model)),
            lens: None,
            software: text(t.get(TrackInfoTag::Software)),
            iso: None,
            focal: None,
            gps: t.get(TrackInfoTag::GpsIso6709).is_some(),
            duration: t
                .get(TrackInfoTag::DurationMs)
                .and_then(|v| v.as_u64())
                .map(|ms| ms as f64 / 1000.0),
            width: t.get(TrackInfoTag::Width).and_then(|v| v.as_u32()),
            height: t.get(TrackInfoTag::Height).and_then(|v| v.as_u32()),
            is_video: true,
        }),
    }
}
