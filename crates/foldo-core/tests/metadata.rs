//! 元数据字段：图片尺寸、EXIF、真实格式；按需读取；条件树 + 变量的完整规则。
//! 测试用的图片在这里现场生成（最小的 PNG 头、带 EXIF 的最小 JPEG）。

use chrono::{Datelike, Local, TimeZone};
use foldo_core::condition::{Condition, MatchCtx};
use foldo_core::config::ConditionDef;
use foldo_core::fields::{self, Value};
use foldo_core::meta::FileCtx;
use foldo_core::{RuleSet, build_plan};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// 只有 IHDR 的 PNG（读尺寸只需要文件头）。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend(13u32.to_be_bytes());
    v.extend(b"IHDR");
    v.extend(width.to_be_bytes());
    v.extend(height.to_be_bytes());
    v.extend([8, 2, 0, 0, 0]);
    v.extend([0, 0, 0, 0]); // CRC（不校验）
    v
}

/// 带 EXIF（Make / Model / DateTimeOriginal / ISO）的最小 JPEG。
fn jpeg_with_exif(make: &str, model: &str, taken: &str, width: u16, height: u16) -> Vec<u8> {
    fn entry(t: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: u32) {
        t.extend(tag.to_le_bytes());
        t.extend(ty.to_le_bytes());
        t.extend(count.to_le_bytes());
        t.extend(value.to_le_bytes());
    }
    let make = format!("{make}\0");
    let model = format!("{model}\0");
    let taken = format!("{taken}\0");
    // 布局：头(8) | IFD0(2+3*12+4) | make | model | ExifIFD(2+2*12+4) | taken
    let ifd0 = 8u32;
    let data = ifd0 + 2 + 3 * 12 + 4;
    let make_off = data;
    let model_off = make_off + make.len() as u32;
    let exif_ifd = model_off + model.len() as u32;
    let taken_off = exif_ifd + 2 + 2 * 12 + 4;

    let mut t = b"II".to_vec();
    t.extend(42u16.to_le_bytes());
    t.extend(ifd0.to_le_bytes());
    t.extend(3u16.to_le_bytes());
    entry(&mut t, 0x010F, 2, make.len() as u32, make_off);
    entry(&mut t, 0x0110, 2, model.len() as u32, model_off);
    entry(&mut t, 0x8769, 4, 1, exif_ifd);
    t.extend(0u32.to_le_bytes());
    t.extend(make.as_bytes());
    t.extend(model.as_bytes());
    t.extend(2u16.to_le_bytes());
    entry(&mut t, 0x9003, 2, taken.len() as u32, taken_off);
    entry(&mut t, 0x8827, 3, 1, 200);
    t.extend(0u32.to_le_bytes());
    t.extend(taken.as_bytes());

    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend(&t);
    let mut v = vec![0xFF, 0xD8, 0xFF, 0xE1];
    v.extend(((app1.len() + 2) as u16).to_be_bytes());
    v.extend(app1);
    // SOF0：精度 8、高、宽、3 个分量
    v.extend([0xFF, 0xC0, 0x00, 0x11, 0x08]);
    v.extend(height.to_be_bytes());
    v.extend(width.to_be_bytes());
    v.extend([0x03, 0x01, 0x22, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]);
    v.extend([0xFF, 0xD9]);
    v
}

fn write(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.join(name);
    fs::write(&p, bytes).unwrap();
    p
}

fn value(f: &FileCtx, key: &str) -> Option<Value> {
    (fields::field(key).unwrap().get)(f)
}

fn test(f: &FileCtx, yaml: &str) -> bool {
    let def: ConditionDef = serde_yaml_ng::from_str(yaml).unwrap();
    Condition::compile(&def)
        .unwrap()
        .matches(f, &mut MatchCtx::new(Local::now()))
}

#[test]
fn image_dimensions_from_png_header() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(tmp.path(), "海报.png", &png(1080, 1920))).unwrap();
    assert_eq!(value(&f, "image.width"), Some(Value::Number(1080.0)));
    assert_eq!(value(&f, "image.height"), Some(Value::Number(1920.0)));
    assert_eq!(
        value(&f, "image.orientation"),
        Some(Value::Text("竖".into()))
    );
    assert_eq!(value(&f, "image.ratio"), Some(Value::Number(0.56)));
    assert_eq!(value(&f, "image.pixels"), Some(Value::Number(2.07)));
    assert!(test(&f, "image.orientation: portrait"));
    assert!(test(&f, "image.height: { '>=': 1080 }"));
    assert!(test(&f, "image.ratio: { between: [0.5, 0.6] }"));
}

#[test]
fn exif_from_jpeg() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(
        tmp.path(),
        "IMG_0001.jpg",
        &jpeg_with_exif("Apple", "iPhone 16 Pro", "2024:05:17 09:30:00", 4032, 3024),
    ))
    .unwrap();
    assert_eq!(value(&f, "exif.make"), Some(Value::Text("Apple".into())));
    assert_eq!(
        value(&f, "exif.model"),
        Some(Value::Text("iPhone 16 Pro".into()))
    );
    assert_eq!(value(&f, "exif.iso"), Some(Value::Number(200.0)));
    assert_eq!(value(&f, "exif.gps"), Some(Value::Bool(false)));
    match value(&f, "exif.taken") {
        Some(Value::Date(d)) => assert_eq!((d.year(), d.month(), d.day()), (2024, 5, 17)),
        other => panic!("没有读到拍摄时间：{other:?}"),
    }
    assert_eq!(value(&f, "image.width"), Some(Value::Number(4032.0)));
    assert_eq!(
        value(&f, "image.orientation"),
        Some(Value::Text("横".into()))
    );
    assert!(test(&f, "exif.model: { contains: iphone }"));
    assert!(
        test(&f, "exif.taken: { within: last_year }")
            || test(&f, "exif.taken: { before: 2025-01-01 }")
    );
    assert!(test(&f, "exif.lens: { exists: false }"));
}

#[test]
fn real_format_ignores_extension() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(tmp.path(), "改过名.dat", &png(10, 10))).unwrap();
    assert_eq!(value(&f, "mime"), Some(Value::Text("image/png".into())));
    assert!(test(&f, "mime: { starts_with: image/ }"));
    // 扩展名不是图片，所以不按图片读尺寸
    assert_eq!(value(&f, "category"), None);
    let txt = FileCtx::from_path(write(tmp.path(), "说明.txt", b"hello")).unwrap();
    assert_eq!(value(&txt, "mime"), None);
}

#[test]
fn metadata_is_read_only_when_needed_and_only_once() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(tmp.path(), "a.png", &png(100, 50))).unwrap();
    assert!(test(&f, "name: { ends_with: .png }"));
    assert!(test(&f, "size: { gt: 0 }"));
    assert_eq!(f.metadata_reads(), 0, "只用基本字段时不应读取元数据");
    assert!(test(&f, "image.width: 100"));
    assert!(test(&f, "image.height: 50"));
    assert!(test(&f, "image.orientation: 横"));
    assert_eq!(f.metadata_reads(), 1, "同一类元数据只读一次");
    // 非图片文件不会去读图片尺寸
    let t = FileCtx::from_path(write(tmp.path(), "b.txt", b"x")).unwrap();
    assert!(test(&t, "image.width: { exists: false }"));
    assert_eq!(t.metadata_reads(), 0);
}

#[test]
fn rule_with_condition_tree_and_variables() {
    let tmp = TempDir::new().unwrap();
    let inbox = tmp.path().canonicalize().unwrap().join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    write(
        &inbox,
        "IMG_0001.jpg",
        &jpeg_with_exif("Apple", "iPhone 16 Pro", "2024:05:17 09:30:00", 4032, 3024),
    );
    write(
        &inbox,
        "IMG_0002-草稿.jpg",
        &jpeg_with_exif("Apple", "iPhone 16 Pro", "2024:05:18 10:00:00", 4032, 3024),
    );
    write(&inbox, "小图.png", &png(200, 200));
    write(&inbox, "竖图.png", &png(1080, 1920));

    let yaml = format!(
        r#"
version: 1
folders:
  - path: '{inbox}'
rules:
  - name: 横版照片
    when:
      all:
        - category: 图片
        - image.width: {{ ">=": 1000 }}
        - image.orientation: 横
        - any:
            - exif.model: {{ contains: iPhone }}
            - name: {{ matches: '^DSC' }}
        - not:
            name: {{ contains: 草稿 }}
    actions:
      - move: "整理/{{exif.taken ?? modified:%Y}}/{{exif.model|slug}}/"
  - name: 竖图
    when:
      all:
        - image.orientation: 竖
        - name: {{ matches: '^(?P<label>.+)\.png$' }}
    actions:
      - rename: "{{match.label}}_{{image.width}}x{{image.height}}.{{ext}}"
"#,
        inbox = inbox.display()
    );
    let rs = RuleSet::from_yaml(&yaml).unwrap();
    let plan = build_plan(&rs, None).unwrap();
    let dst = |name: &str| {
        plan.ops
            .iter()
            .rfind(|o| o.origin.file_name().unwrap() == name)
            .and_then(|o| o.dst.clone())
    };
    assert_eq!(
        dst("IMG_0001.jpg"),
        Some(inbox.join("整理/2024/iphone-16-pro/IMG_0001.jpg"))
    );
    assert_eq!(dst("IMG_0002-草稿.jpg"), None, "草稿被 not 排除");
    assert_eq!(dst("小图.png"), None, "宽度不够，也不是竖图");
    assert_eq!(dst("竖图.png"), Some(inbox.join("竖图_1080x1920.png")));

    // when 可以写回 YAML 再读出来
    let file = foldo_core::parse_rule_file(&yaml).unwrap();
    let again = foldo_core::parse_rule_file(&file.to_yaml().unwrap()).unwrap();
    assert_eq!(again.to_yaml().unwrap(), file.to_yaml().unwrap());
}

#[test]
fn inspect_lists_present_fields() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(
        tmp.path(),
        "IMG_9.jpg",
        &jpeg_with_exif("Canon", "EOS R5", "2023:12:31 23:59:00", 8192, 5464),
    ))
    .unwrap();
    let present: Vec<&str> = fields::inspect(&f)
        .into_iter()
        .filter(|(_, v)| v.is_some())
        .map(|(d, _)| d.key)
        .collect();
    for k in [
        "name",
        "size",
        "category",
        "mime",
        "modified",
        "image.width",
        "exif.make",
        "exif.taken",
    ] {
        assert!(present.contains(&k), "缺少 {k}：{present:?}");
    }
    assert!(!present.contains(&"video.duration"));
    let _ = Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0);
}

// ---------- 第 3 步：音频、PDF、Office、内容、下载来源 ----------

/// 两页、带标题和作者的最小 PDF（交叉引用表的偏移量按实际位置计算）。
fn pdf(title: &str, author: &str) -> Vec<u8> {
    let objs = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        "<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_string(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_string(),
        format!("<< /Title ({title}) /Author ({author}) >>"),
    ];
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, o) in objs.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{o}\nendobj\n", i + 1).as_bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objs.len() + 1).as_bytes());
    for off in offsets {
        out.extend(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R /Info 5 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objs.len() + 1
        )
        .as_bytes(),
    );
    out
}

fn docx(path: &Path) {
    use std::io::Write;
    let mut w = zip::ZipWriter::new(fs::File::create(path).unwrap());
    let opt = zip::write::SimpleFileOptions::default();
    w.start_file("docProps/core.xml", opt).unwrap();
    w.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>&#24180;&#24230; &amp; &#24635;&#32467;</dc:title><dc:creator>Zhang San</dc:creator></cp:coreProperties>"#).unwrap();
    w.start_file("docProps/app.xml", opt).unwrap();
    w.write_all(br#"<?xml version="1.0" encoding="UTF-8"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties"><Pages>12</Pages><Company>Wonvy</Company></Properties>"#).unwrap();
    w.finish().unwrap();
}

/// 1 秒的单声道 WAV，带 RIFF INFO 标签（标题、艺术家）。
fn wav(title: &str, artist: &str) -> Vec<u8> {
    fn chunk(id: &[u8], body: &[u8]) -> Vec<u8> {
        let mut c = id.to_vec();
        c.extend((body.len() as u32).to_le_bytes());
        c.extend(body);
        if body.len() % 2 == 1 {
            c.push(0);
        }
        c
    }
    let mut fmt = Vec::new();
    fmt.extend(1u16.to_le_bytes()); // PCM
    fmt.extend(1u16.to_le_bytes()); // 单声道
    fmt.extend(8000u32.to_le_bytes());
    fmt.extend(16000u32.to_le_bytes());
    fmt.extend(2u16.to_le_bytes());
    fmt.extend(16u16.to_le_bytes());
    let mut info = b"INFO".to_vec();
    info.extend(chunk(b"INAM", format!("{title}\0").as_bytes()));
    info.extend(chunk(b"IART", format!("{artist}\0").as_bytes()));
    let mut body = b"WAVE".to_vec();
    body.extend(chunk(b"fmt ", &fmt));
    body.extend(chunk(b"data", &vec![0u8; 16000]));
    body.extend(chunk(b"LIST", &info));
    chunk(b"RIFF", &body)
}

#[test]
fn pdf_pages_and_info() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(
        tmp.path(),
        "报告.pdf",
        &pdf("Quarterly Report", "Foldo Team"),
    ))
    .unwrap();
    assert_eq!(value(&f, "pdf.pages"), Some(Value::Number(2.0)));
    assert_eq!(
        value(&f, "pdf.title"),
        Some(Value::Text("Quarterly Report".into()))
    );
    assert_eq!(
        value(&f, "pdf.author"),
        Some(Value::Text("Foldo Team".into()))
    );
    assert!(test(&f, "pdf.pages: { '>=': 2 }"));
}

#[test]
fn office_document_properties() {
    let tmp = TempDir::new().unwrap();
    let p = tmp.path().join("总结.docx");
    docx(&p);
    let f = FileCtx::from_path(&p).unwrap();
    assert_eq!(
        value(&f, "doc.title"),
        Some(Value::Text("年度 & 总结".into()))
    );
    assert_eq!(
        value(&f, "doc.author"),
        Some(Value::Text("Zhang San".into()))
    );
    assert_eq!(value(&f, "doc.company"), Some(Value::Text("Wonvy".into())));
    assert_eq!(value(&f, "doc.pages"), Some(Value::Number(12.0)));
    assert!(test(&f, "doc.title: { contains: 总结 }"));
}

#[test]
fn audio_duration_and_tags() {
    let tmp = TempDir::new().unwrap();
    let f =
        FileCtx::from_path(write(tmp.path(), "录音.wav", &wav("Weekly Sync", "Foldo"))).unwrap();
    assert_eq!(value(&f, "audio.duration"), Some(Value::Number(1.0)));
    assert_eq!(
        value(&f, "audio.title"),
        Some(Value::Text("Weekly Sync".into()))
    );
    assert_eq!(value(&f, "audio.artist"), Some(Value::Text("Foldo".into())));
    assert!(test(&f, "audio.duration: { lt: 1m }"));
}

#[test]
fn text_content() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(
        tmp.path(),
        "说明.md",
        "# 合同\n编号 HT-2026-001\n".as_bytes(),
    ))
    .unwrap();
    assert!(test(&f, "content: { matches: 'HT-(?P<year>\\d{4})' }"));
    assert!(test(&f, "content: { contains: 合同 }"));
    // 非文本类文件不读内容
    let bin = FileCtx::from_path(write(tmp.path(), "a.png", &png(1, 1))).unwrap();
    assert!(test(&bin, "content: { exists: false }"));
}

#[cfg(target_os = "macos")]
#[test]
fn download_source_from_finder_metadata() {
    let tmp = TempDir::new().unwrap();
    let p = write(tmp.path(), "发票.pdf", b"%PDF-1.4");
    let mut buf = Vec::new();
    plist::to_writer_binary(
        &mut buf,
        &vec![
            "https://www.mail.qq.com/cgi-bin/download?id=1",
            "https://mail.qq.com/",
        ],
    )
    .unwrap();
    xattr::set(&p, "com.apple.metadata:kMDItemWhereFroms", &buf).unwrap();
    let f = FileCtx::from_path(&p).unwrap();
    assert_eq!(
        value(&f, "source.domain"),
        Some(Value::Text("mail.qq.com".into()))
    );
    assert!(test(&f, "source.url: { contains: download }"));
}

#[cfg(windows)]
#[test]
fn download_source_from_zone_identifier() {
    let tmp = TempDir::new().unwrap();
    let p = write(tmp.path(), "发票.pdf", b"%PDF-1.4");
    let mut ads = p.as_os_str().to_owned();
    ads.push(":Zone.Identifier");
    fs::write(PathBuf::from(ads), "[ZoneTransfer]\r\nZoneId=3\r\nReferrerUrl=https://mail.qq.com/\r\nHostUrl=https://dl.mail.qq.com/file?id=1\r\n").unwrap();
    let f = FileCtx::from_path(&p).unwrap();
    assert_eq!(
        value(&f, "source.domain"),
        Some(Value::Text("dl.mail.qq.com".into()))
    );
}

#[test]
fn files_without_source_have_no_source_fields() {
    let tmp = TempDir::new().unwrap();
    let f = FileCtx::from_path(write(tmp.path(), "本地.txt", b"x")).unwrap();
    assert!(test(&f, "source.url: { exists: false }"));
}

#[test]
fn duplicate_files_keep_the_oldest_copy() {
    use foldo_core::{RuleSet, build_plan};
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().canonicalize().unwrap();
    let write = |name: &str, body: &str, mtime: i64| {
        let p = dir.join(name);
        std::fs::write(&p, body).unwrap();
        filetime::set_file_mtime(&p, filetime::FileTime::from_unix_time(mtime, 0)).unwrap();
        p
    };
    write("原件.txt", "同样的内容", 1_600_000_000);
    write("原件 (1).txt", "同样的内容", 1_700_000_000);
    write("原件 副本.txt", "同样的内容", 1_650_000_000);
    write("同样大小.txt", "不一样内容", 1_650_000_000); // 大小相同（UTF-8 字节数一样）但内容不同
    write("别的.txt", "完全不同的内容，长度也不一样", 1_650_000_000);

    let rs = RuleSet::from_yaml(&format!(
        "version: 2\nfolders:\n  - path: '{}'\n    rules: [重复]\nrules:\n  - name: 重复\n    when: {{ duplicate: true }}\n    actions: [{{ move: '重复/{{duplicate.of}}/' }}]\n",
        dir.display()
    ))
    .unwrap();
    let plan = build_plan(&rs, None).unwrap();
    let mut moved: Vec<String> = plan
        .ops
        .iter()
        .map(|o| {
            let d = o.dst.as_ref().unwrap();
            let folder = d.parent().unwrap().file_name().unwrap().to_string_lossy();
            format!("{folder}/{}", d.file_name().unwrap().to_string_lossy())
        })
        .collect();
    moved.sort();
    // 最早的“原件.txt”留下；另外两份副本按原件名归到一起
    assert_eq!(
        moved,
        vec!["原件.txt/原件 (1).txt", "原件.txt/原件 副本.txt"]
    );
}
