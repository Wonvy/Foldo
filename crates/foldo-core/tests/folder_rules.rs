//! 文件夹规则：按文件夹类型、内容类型、里面的项目名分类，整个文件夹一起移动、压缩、撤销。

use foldo_core::journal::status;
use foldo_core::{
    Journal, RuleSet, SkipKind, build_plan, build_plan_for_paths, execute, undo_batch,
};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn touch(dir: &Path, rel: &str) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, rel).unwrap();
    // 把修改时间设到很久以前，避开缓冲期
    filetime::set_file_mtime(&p, filetime::FileTime::from_unix_time(1_600_000_000, 0)).unwrap();
    p
}

fn q(p: &Path) -> String {
    format!("'{}'", p.display())
}

struct Env {
    _tmp: TempDir,
    inbox: PathBuf,
}

fn env() -> Env {
    let tmp = TempDir::new().unwrap();
    let inbox = tmp.path().canonicalize().unwrap().join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    // 第一层子文件夹
    touch(&inbox, "旅行照片/1.jpg");
    touch(&inbox, "旅行照片/2.jpg");
    touch(&inbox, "旅行照片/3.heic");
    touch(&inbox, "旅行照片/说明.txt");
    touch(&inbox, "my-app/package.json");
    touch(&inbox, "my-app/src/index.js");
    touch(&inbox, "杂项/a.pdf");
    touch(&inbox, "杂项/b.mp3");
    touch(&inbox, "VS 工程/Demo.sln");
    fs::create_dir_all(inbox.join("空的")).unwrap();
    touch(&inbox, "散落.pdf");
    Env { _tmp: tmp, inbox }
}

fn rules(e: &Env, recursive: bool) -> RuleSet {
    let yaml = format!(
        r#"
version: 2
folders:
  - path: {inbox}
    recursive: {recursive}
    rules: [相册, 代码, 解决方案, 空文件夹, 文档]
rules:
  - name: 相册
    target: folder
    when: {{ folder.content: 图片 }}
    actions: [{{ move: '相册/' }}]
  - name: 代码
    target: folder
    when: {{ all: [{{ folder.kind: 代码工程 }}, {{ folder.names: {{ not_contains: README }} }}] }}
    actions: [{{ move: '代码/' }}]
  - name: 解决方案
    target: folder
    when: {{ folder.names: {{ glob: '*.sln' }} }}
    actions: [{{ move: '代码/' }}]
  - name: 空文件夹
    target: folder
    when: {{ empty: true }}
    actions: [{{ move: '空/' }}]
  - name: 文档
    when: {{ category: 文档 }}
    actions: [{{ move: '文档/' }}]
"#,
        inbox = q(&e.inbox)
    );
    RuleSet::from_yaml(&yaml).unwrap_or_else(|err| panic!("{err}\n{yaml}"))
}

fn dst_of(plan: &foldo_core::Plan, name: &str) -> Option<PathBuf> {
    plan.ops
        .iter()
        .rfind(|o| o.origin.file_name().unwrap() == name)
        .and_then(|o| o.dst.clone())
}

#[test]
fn folders_are_classified_by_kind_content_and_names() {
    let e = env();
    let plan = build_plan(&rules(&e, false), None).unwrap();
    assert_eq!(
        dst_of(&plan, "旅行照片"),
        Some(e.inbox.join("相册/旅行照片"))
    );
    assert_eq!(dst_of(&plan, "my-app"), Some(e.inbox.join("代码/my-app")));
    assert_eq!(dst_of(&plan, "VS 工程"), Some(e.inbox.join("代码/VS 工程")));
    assert_eq!(dst_of(&plan, "空的"), Some(e.inbox.join("空/空的")));
    // 内容混合、没有规则匹配的文件夹作为整体保留
    assert!(dst_of(&plan, "杂项").is_none());
    assert!(
        plan.skipped
            .iter()
            .any(|s| s.path.ends_with("杂项") && s.kind == SkipKind::Folder)
    );
    // 文件规则照常处理散落的文件，不会拆开文件夹
    assert_eq!(
        dst_of(&plan, "散落.pdf"),
        Some(e.inbox.join("文档/散落.pdf"))
    );
    assert!(dst_of(&plan, "a.pdf").is_none());
    // 文件夹的分组标记为“文件夹”
    assert!(
        plan.ops
            .iter()
            .filter(|o| o.origin.ends_with("my-app"))
            .all(|o| o.category.as_deref() == Some("文件夹"))
    );
}

#[test]
fn files_inside_a_moved_folder_are_left_to_the_folder_rule() {
    let e = env();
    // 包含子文件夹时：杂项里的 a.pdf 由文件规则处理；被文件夹规则移动的文件夹里的文件不再单独处理
    let plan = build_plan(&rules(&e, true), None).unwrap();
    assert_eq!(dst_of(&plan, "a.pdf"), Some(e.inbox.join("文档/a.pdf")));
    assert_eq!(
        dst_of(&plan, "旅行照片"),
        Some(e.inbox.join("相册/旅行照片"))
    );
    assert!(dst_of(&plan, "说明.txt").is_none(), "{:?}", plan.ops);
    assert!(!plan.skipped.iter().any(|s| s.path.ends_with("说明.txt")));
}

#[test]
fn moving_folders_then_undo() {
    let e = env();
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(
        &build_plan(&rules(&e, false), None).unwrap(),
        &journal,
        "test",
    )
    .unwrap();
    assert!(report.failed.is_empty(), "{report:?}");
    assert!(e.inbox.join("相册/旅行照片/1.jpg").exists());
    assert!(e.inbox.join("代码/my-app/src/index.js").exists());
    assert!(!e.inbox.join("my-app").exists());

    let undo = undo_batch(&journal, report.batch_id).unwrap();
    assert!(undo.failed.is_empty(), "{undo:?}");
    assert!(e.inbox.join("旅行照片/1.jpg").exists());
    assert!(e.inbox.join("my-app/src/index.js").exists());
    assert!(!e.inbox.join("相册").exists());
    assert_eq!(
        journal.batch(report.batch_id).unwrap().unwrap().status,
        status::UNDONE
    );
}

#[test]
fn compress_a_folder_and_drop_a_folder() {
    let e = env();
    let rs = RuleSet::from_yaml(&format!(
        "version: 2\nfolders:\n  - path: {inbox}\n    rules: [打包]\nrules:\n  - name: 打包\n    target: folder\n    when: {{ folder.exts: jpg }}\n    actions: [{{ compress: '{{name}}.zip' }}]\n",
        inbox = q(&e.inbox)
    ))
    .unwrap();
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(report.done, 1, "{report:?}");
    let zip = e.inbox.join("旅行照片.zip");
    let mut archive = zip::ZipArchive::new(fs::File::open(&zip).unwrap()).unwrap();
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
        .collect();
    assert!(names.contains(&"旅行照片/1.jpg".to_string()), "{names:?}");
    // 原文件夹保留；再扫描不会重复压缩
    assert!(e.inbox.join("旅行照片").is_dir());
    assert!(build_plan(&rs, None).unwrap().ops.is_empty());

    // 拖入文件夹：交给文件夹规则
    fs::remove_file(&zip).unwrap();
    let dropped = build_plan_for_paths(
        &rs,
        &[e.inbox.join("旅行照片"), e.inbox.join("杂项")],
        chrono::Local::now(),
    );
    assert_eq!(dropped.ops.len(), 1);
    assert!(
        dropped
            .skipped
            .iter()
            .any(|s| s.path.ends_with("杂项") && s.kind == SkipKind::Folder)
    );
}

#[test]
fn extract_is_rejected_for_folder_rules() {
    let e = env();
    let yaml = format!(
        "version: 2\nfolders:\n  - path: {}\nrules:\n  - name: x\n    target: folder\n    actions: [{{ extract: '{{name}}/' }}]\n",
        q(&e.inbox)
    );
    assert!(RuleSet::from_yaml(&yaml).is_err());
}

#[test]
fn recently_changed_folders_wait_for_the_buffer() {
    let e = env();
    let yaml = format!(
        "version: 2\nsettings:\n  buffer: 1h\nfolders:\n  - path: {}\n    rules: [相册]\nrules:\n  - name: 相册\n    target: folder\n    when: {{ folder.content: 图片 }}\n    actions: [{{ move: '相册/' }}]\n",
        q(&e.inbox)
    );
    let plan = build_plan(&RuleSet::from_yaml(&yaml).unwrap(), None).unwrap();
    // 文件夹刚建好（可能还在下载或解压），先不动
    assert!(plan.ops.is_empty());
    assert!(
        plan.skipped
            .iter()
            .any(|s| s.path.ends_with("旅行照片") && s.kind == SkipKind::Buffer)
    );
}
