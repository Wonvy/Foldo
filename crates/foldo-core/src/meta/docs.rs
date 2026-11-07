//! 音频标签、PDF 信息、Office 文档属性、下载来源、文本内容。

use std::io::Read;
use std::path::Path;

// ---------- 音频（lofty） ----------

#[derive(Debug, Clone, Default)]
pub struct AudioMeta {
    /// 秒
    pub duration: Option<f64>,
    /// kbps
    pub bitrate: Option<u32>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub genre: Option<String>,
    pub year: Option<u32>,
}

pub fn audio(path: &Path) -> Option<AudioMeta> {
    use lofty::prelude::*;
    let tagged = lofty::read_from_path(path).ok()?;
    let props = tagged.properties();
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());
    let s = |v: Option<std::borrow::Cow<'_, str>>| {
        v.map(|c| c.trim().to_string()).filter(|x| !x.is_empty())
    };
    let secs = props.duration().as_secs_f64();
    Some(AudioMeta {
        duration: (secs > 0.0).then_some((secs * 10.0).round() / 10.0),
        bitrate: props.audio_bitrate(),
        title: tag.and_then(|t| s(t.title())),
        artist: tag.and_then(|t| s(t.artist())),
        album: tag.and_then(|t| s(t.album())),
        genre: tag.and_then(|t| s(t.genre())),
        year: tag.and_then(|t| t.date()).map(|d| d.year as u32),
    })
}

// ---------- PDF（lopdf，只读元数据，不加载整个文档） ----------

#[derive(Debug, Clone, Default)]
pub struct PdfMeta {
    pub pages: Option<u32>,
    pub title: Option<String>,
    pub author: Option<String>,
}

pub fn pdf(path: &Path) -> Option<PdfMeta> {
    let m = lopdf::Document::load_metadata(path).ok()?;
    let clean = |s: Option<String>| s.map(|x| x.trim().to_string()).filter(|x| !x.is_empty());
    Some(PdfMeta {
        pages: (m.page_count > 0).then_some(m.page_count),
        title: clean(m.title),
        author: clean(m.author),
    })
}

// ---------- Office 文档（docx / xlsx / pptx 的 docProps） ----------

#[derive(Debug, Clone, Default)]
pub struct OfficeMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub company: Option<String>,
    pub pages: Option<u32>,
    pub slides: Option<u32>,
}

/// 读取 XML 中指定元素的文本（按本地名，忽略命名空间前缀）。实体（如 &amp;）会被还原。
fn xml_texts(xml: &str, wanted: &[&str]) -> Vec<(String, String)> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut out = Vec::new();
    let mut current: Option<(String, String)> = None;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                let local = e.local_name();
                let name: &str = local.as_ref();
                current = wanted
                    .contains(&name)
                    .then(|| (name.to_string(), String::new()));
            }
            Ok(Event::Text(t)) => {
                if let Some((_, buf)) = current.as_mut() {
                    buf.push_str(&t.xml10_content());
                }
            }
            Ok(Event::GeneralRef(r)) => {
                if let Some((_, buf)) = current.as_mut() {
                    let name: &str = &r.xml10_content();
                    let ch = match name {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        n if n.starts_with("#x") => u32::from_str_radix(&n[2..], 16)
                            .ok()
                            .and_then(char::from_u32),
                        n if n.starts_with('#') => n[1..].parse().ok().and_then(char::from_u32),
                        _ => None,
                    };
                    if let Some(c) = ch {
                        buf.push(c);
                    }
                }
            }
            Ok(Event::End(_)) => {
                if let Some((name, text)) = current.take()
                    && !text.trim().is_empty()
                {
                    out.push((name, text.trim().to_string()));
                }
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    out
}

pub fn office(path: &Path) -> Option<OfficeMeta> {
    let file = std::fs::File::open(path).ok()?;
    let mut zip = zip::ZipArchive::new(file).ok()?;
    let mut read = |name: &str| -> Option<String> {
        let entry = zip.by_name(name).ok()?;
        let mut s = String::new();
        entry.take(1 << 20).read_to_string(&mut s).ok()?;
        Some(s)
    };
    let core = read("docProps/core.xml").unwrap_or_default();
    let app = read("docProps/app.xml").unwrap_or_default();
    if core.is_empty() && app.is_empty() {
        return None;
    }
    let get = |list: &[(String, String)], key: &str| {
        list.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    };
    let c = xml_texts(&core, &["title", "creator"]);
    let a = xml_texts(&app, &["Company", "Pages", "Slides"]);
    Some(OfficeMeta {
        title: get(&c, "title"),
        author: get(&c, "creator"),
        company: get(&a, "Company"),
        pages: get(&a, "Pages").and_then(|v| v.parse().ok()),
        slides: get(&a, "Slides").and_then(|v| v.parse().ok()),
    })
}

// ---------- 下载来源 ----------

/// 下载网址：macOS 读 Finder 的“来源”（kMDItemWhereFroms），Linux 读 user.xdg.origin.url，
/// Windows 读下载标记（Zone.Identifier 里的 HostUrl / ReferrerUrl）。
pub fn source_url(path: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let raw = xattr::get(path, "com.apple.metadata:kMDItemWhereFroms").ok()??;
        let urls: Vec<String> = plist::from_bytes(&raw).ok()?;
        urls.into_iter().find(|u| !u.trim().is_empty())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let raw = xattr::get(path, "user.xdg.origin.url").ok()??;
        String::from_utf8(raw).ok().filter(|s| !s.trim().is_empty())
    }
    #[cfg(windows)]
    {
        let mut ads = path.as_os_str().to_owned();
        ads.push(":Zone.Identifier");
        let text = std::fs::read_to_string(std::path::PathBuf::from(ads)).ok()?;
        let find = |key: &str| {
            text.lines()
                .find_map(|l| l.strip_prefix(key))
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty() && v != "about:internet")
        };
        find("HostUrl=").or_else(|| find("ReferrerUrl="))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = path;
        None
    }
}

/// `https://www.example.com:8080/a/b` → `example.com`
pub fn domain(url: &str) -> Option<String> {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next()?;
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    let host = host.split(':').next()?.trim_start_matches("www.");
    (!host.is_empty()).then(|| host.to_lowercase())
}

// ---------- 文本内容 ----------

const TEXT_EXTS: &[&str] = &[
    "txt", "md", "markdown", "csv", "tsv", "json", "xml", "html", "htm", "yaml", "yml", "toml",
    "ini", "cfg", "conf", "log", "srt", "vtt", "rtf", "tex", "py", "js", "ts", "rs", "go", "java",
    "c", "cpp", "h", "sh", "sql",
];

/// 纯文本文件的内容（最多前 1 MB，按 UTF-8 解码，无法解码的字节替换掉）。
pub fn content(path: &Path, ext: &str) -> Option<String> {
    if !TEXT_EXTS.contains(&ext.to_lowercase().as_str()) {
        return None;
    }
    let mut buf = Vec::new();
    std::fs::File::open(path)
        .ok()?
        .take(1 << 20)
        .read_to_end(&mut buf)
        .ok()?;
    let s = String::from_utf8_lossy(&buf).into_owned();
    (!s.trim().is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domains() {
        assert_eq!(
            domain("https://www.Example.com:8080/a?b").as_deref(),
            Some("example.com")
        );
        assert_eq!(
            domain("http://user@mail.qq.com/x").as_deref(),
            Some("mail.qq.com")
        );
        assert_eq!(
            domain("ftp://files.server.cn").as_deref(),
            Some("files.server.cn")
        );
        assert_eq!(domain("").as_deref(), None);
    }
}
