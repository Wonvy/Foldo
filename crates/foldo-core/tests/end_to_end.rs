//! 端到端测试：在临时目录里构造文件 → 生成计划 → 执行 → 撤销。
//! 注意：这里不执行任何 trash 操作，避免把测试文件真的放进系统回收站。

use filetime::FileTime;
use foldo_core::journal::status;
use foldo_core::{Journal, OpKind, RuleSet, build_plan, execute, undo_batch};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

/// 2024-05-15 12:00:00 UTC，任何时区下都还在 5 月。
const MAY_2024: i64 = 1_715_774_400;

fn touch(dir: &Path, rel: &str, content: &str) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, content).unwrap();
    p
}

fn set_mtime(p: &Path, unix: i64) {
    filetime::set_file_mtime(p, FileTime::from_unix_time(unix, 0)).unwrap();
}

fn days_ago(days: u64) -> i64 {
    (SystemTime::now() - Duration::from_secs(days * 86_400))
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64
}

/// YAML 单引号字符串里的路径（兼容 Windows 反斜杠）。
fn q(p: &Path) -> String {
    format!("'{}'", p.display())
}

fn rules(yaml: &str) -> RuleSet {
    RuleSet::from_yaml(yaml).unwrap_or_else(|e| panic!("规则解析失败: {e}\n{yaml}"))
}

struct Env {
    _tmp: TempDir,
    inbox: PathBuf,
    out: PathBuf,
}

fn env() -> Env {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let inbox = root.join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    Env {
        out: root.join("out"),
        inbox,
        _tmp: tmp,
    }
}

#[test]
fn move_by_extension_then_undo() {
    let e = env();
    let a = touch(&e.inbox, "a.jpg", "aaa");
    set_mtime(&a, MAY_2024);
    let b = touch(&e.inbox, "b.PNG", "bb");
    set_mtime(&b, MAY_2024);
    let c = touch(&e.inbox, "c.txt", "c");
    let hidden = touch(&e.inbox, ".secret.jpg", "h");
    let partial = touch(&e.inbox, "movie.jpg.crdownload", "p");

    let rs = rules(&format!(
        r#"
version: 1
rules:
  - name: images
    locations: [{inbox}]
    conditions:
      - extension: [jpg, png]
    actions:
      - move: {dest}
"#,
        inbox = q(&e.inbox),
        dest = q(&e.out.join("{modified:%Y}").join("{modified:%m}"))
    ));

    let plan = build_plan(&rs, None).unwrap();
    assert_eq!(plan.ops.len(), 2, "{plan:#?}");
    assert_eq!(plan.matched, 2);
    // 生成计划不能改动磁盘
    assert!(a.exists() && b.exists() && !e.out.exists());

    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&plan, &journal, "test").unwrap();
    assert_eq!(report.done, 2);
    assert!(report.failed.is_empty());

    let month_dir = e.out.join("2024").join("05");
    assert_eq!(fs::read_to_string(month_dir.join("a.jpg")).unwrap(), "aaa");
    assert!(month_dir.join("b.PNG").exists());
    assert!(!a.exists() && !b.exists());
    // 不匹配的、隐藏的、下载中的文件都不动
    assert!(c.exists() && hidden.exists() && partial.exists());

    let undo = undo_batch(&journal, report.batch_id).unwrap();
    assert_eq!(undo.restored, 2);
    assert!(undo.failed.is_empty());
    assert_eq!(fs::read_to_string(&a).unwrap(), "aaa");
    assert!(b.exists());
    // 执行时新建的目录也一并清理
    assert!(!e.out.exists());
    assert_eq!(
        journal.batch(report.batch_id).unwrap().unwrap().status,
        status::UNDONE
    );
    // 不能重复撤销
    assert!(undo_batch(&journal, report.batch_id).is_err());
}

#[test]
fn conflict_policies() {
    let e = env();
    touch(&e.inbox, "r.txt", "new");
    touch(&e.out, "r.txt", "old");

    let make = |policy: &str| {
        rules(&format!(
            "version: 1\nrules:\n  - name: t\n    locations: [{}]\n    actions:\n      - move: {{ to: {}, on_conflict: {policy} }}\n",
            q(&e.inbox),
            q(&e.out)
        ))
    };

    let plan = build_plan(&make("rename"), None).unwrap();
    assert_eq!(
        plan.ops[0].dst.as_deref(),
        Some(e.out.join("r (1).txt").as_path())
    );

    let plan = build_plan(&make("skip"), None).unwrap();
    assert!(plan.ops.is_empty());
    assert_eq!(plan.skipped.len(), 1);

    let plan = build_plan(&make("overwrite"), None).unwrap();
    let kinds: Vec<OpKind> = plan.ops.iter().map(|o| o.kind).collect();
    assert_eq!(kinds, [OpKind::Trash, OpKind::Move]);
    assert_eq!(plan.ops[0].src, e.out.join("r.txt"));
}

#[test]
fn files_in_one_plan_do_not_collide() {
    let e = env();
    let other = e.inbox.parent().unwrap().join("inbox2");
    touch(&e.inbox, "x.txt", "1");
    touch(&other, "x.txt", "2");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: t\n    locations: [{}, {}]\n    actions:\n      - move: {}\n",
        q(&e.inbox),
        q(&other),
        q(&e.out)
    ));
    let plan = build_plan(&rs, None).unwrap();
    let dsts: Vec<_> = plan.ops.iter().map(|o| o.dst.clone().unwrap()).collect();
    assert_eq!(dsts, [e.out.join("x.txt"), e.out.join("x (1).txt")]);

    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&plan, &journal, "test").unwrap();
    assert_eq!(report.done, 2);
    assert_eq!(fs::read_to_string(e.out.join("x (1).txt")).unwrap(), "2");
}

#[test]
fn first_matching_rule_wins() {
    let e = env();
    touch(&e.inbox, "doc.txt", "");
    let rs = rules(&format!(
        r#"
version: 1
rules:
  - name: first
    locations: [{inbox}]
    conditions: [{{ extension: txt }}]
    actions: [{{ move: {a} }}]
  - name: second
    locations: [{inbox}]
    actions: [{{ move: {b} }}]
"#,
        inbox = q(&e.inbox),
        a = q(&e.out.join("a")),
        b = q(&e.out.join("b")),
    ));
    let plan = build_plan(&rs, None).unwrap();
    assert_eq!(plan.ops.len(), 1);
    assert_eq!(plan.ops[0].rule, "first");

    // 指定只运行 second 时，second 会处理它
    let plan = build_plan(&rs, Some("second")).unwrap();
    assert_eq!(plan.ops[0].rule, "second");
    assert!(build_plan(&rs, Some("nope")).is_err());
}

#[test]
fn rename_then_move_with_regex_captures() {
    let e = env();
    let src = touch(&e.inbox, "IMG_20240517_123.JPG", "img");
    let rs = rules(&format!(
        r#"
version: 1
rules:
  - name: photos
    locations: [{inbox}]
    conditions:
      - name: {{ regex: '^img_(?P<date>\d{{8}})_(\d+)' }}
    actions:
      - rename: "photo_{{regex.date}}_{{regex.2}}.{{ext|lower}}"
      - move: {out}
"#,
        inbox = q(&e.inbox),
        out = q(&e.out),
    ));
    let plan = build_plan(&rs, None).unwrap();
    let kinds: Vec<OpKind> = plan.ops.iter().map(|o| o.kind).collect();
    assert_eq!(kinds, [OpKind::Rename, OpKind::Move]);

    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&plan, &journal, "test").unwrap();
    assert_eq!(report.done, 2, "{:?}", report.failed);
    let final_path = e.out.join("photo_20240517_123.jpg");
    assert_eq!(fs::read_to_string(&final_path).unwrap(), "img");

    undo_batch(&journal, report.batch_id).unwrap();
    assert_eq!(fs::read_to_string(&src).unwrap(), "img");
    assert!(!final_path.exists());
    assert!(!e.inbox.join("photo_20240517_123.jpg").exists());
}

#[test]
fn copy_then_undo_keeps_modified_copies() {
    let e = env();
    let src = touch(&e.inbox, "keep.txt", "v1");
    let other = touch(&e.inbox, "edit.txt", "v1");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: backup\n    locations: [{}]\n    actions:\n      - copy: {}\n",
        q(&e.inbox),
        q(&e.out)
    ));
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(report.done, 2);
    assert!(src.exists() && other.exists());
    assert!(e.out.join("keep.txt").exists());

    // 用户改了其中一个副本，撤销时不能删掉它
    fs::write(e.out.join("edit.txt"), "user changed this").unwrap();
    let undo = undo_batch(&journal, report.batch_id).unwrap();
    assert_eq!(undo.restored, 1);
    assert_eq!(undo.failed.len(), 1);
    assert!(!e.out.join("keep.txt").exists());
    assert!(e.out.join("edit.txt").exists());
    assert_eq!(
        journal.batch(report.batch_id).unwrap().unwrap().status,
        status::PARTIALLY_UNDONE
    );
}

#[test]
fn relative_destination_is_idempotent_when_recursive() {
    let e = env();
    touch(&e.inbox, "a.txt", "");
    touch(&e.inbox, "nested/b.txt", "");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: t\n    locations: [{}]\n    recursive: true\n    conditions: [{{ extension: txt }}]\n    actions: [{{ move: 'sorted/' }}]\n",
        q(&e.inbox)
    ));
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(report.done, 2);
    assert!(e.inbox.join("sorted/a.txt").exists());
    assert!(e.inbox.join("sorted/b.txt").exists());

    let again = build_plan(&rs, None).unwrap();
    assert!(again.ops.is_empty(), "{again:#?}");
    assert_eq!(again.in_place, 2);
}

#[test]
fn time_size_and_logic_conditions() {
    let e = env();
    let old_big = touch(&e.inbox, "old_big.log", &"x".repeat(2_000));
    let old_small = touch(&e.inbox, "old_small.log", "x");
    let new_big = touch(&e.inbox, "new_big.log", &"x".repeat(2_000));
    let keep = touch(&e.inbox, "keep-old.log", &"x".repeat(2_000));
    for p in [&old_big, &old_small, &keep] {
        set_mtime(p, days_ago(40));
    }
    set_mtime(&new_big, days_ago(1));

    let rs = rules(&format!(
        r#"
version: 1
rules:
  - name: t
    locations: [{inbox}]
    conditions:
      - modified: {{ older_than: 30d }}
      - any:
          - size: {{ gt: 1KB }}
          - name: "*_small.*"
      - not: {{ name: {{ starts_with: keep }} }}
    actions: [{{ move: {out} }}]
"#,
        inbox = q(&e.inbox),
        out = q(&e.out)
    ));
    let plan = build_plan(&rs, None).unwrap();
    let mut moved: Vec<String> = plan
        .ops
        .iter()
        .map(|o| o.src.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    moved.sort();
    assert_eq!(moved, ["old_big.log", "old_small.log"]);
}

#[test]
fn executor_never_overwrites_files_that_appear_after_planning() {
    let e = env();
    let src = touch(&e.inbox, "late.txt", "mine");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: t\n    locations: [{}]\n    actions: [{{ move: {} }}]\n",
        q(&e.inbox),
        q(&e.out)
    ));
    let plan = build_plan(&rs, None).unwrap();
    touch(&e.out, "late.txt", "someone else");

    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&plan, &journal, "test").unwrap();
    assert_eq!(report.done, 0);
    // 执行时才出现的冲突算“跳过”，不算“失败”
    assert_eq!(report.skipped.len(), 1);
    assert!(report.failed.is_empty());
    assert_eq!(fs::read_to_string(&src).unwrap(), "mine");
    assert_eq!(
        fs::read_to_string(e.out.join("late.txt")).unwrap(),
        "someone else"
    );
}

#[test]
fn invalid_rule_files_are_rejected() {
    let e = env();
    let loc = q(&e.inbox);
    let cases = [
        ("版本", "version: 3\nrules: []\n".to_string()),
        (
            "未知字段",
            format!(
                "version: 1\nrules:\n  - name: t\n    locations: [{loc}]\n    acitons: [trash]\n"
            ),
        ),
        (
            "没有动作",
            format!("version: 1\nrules:\n  - name: t\n    locations: [{loc}]\n    actions: []\n"),
        ),
        (
            "正则",
            format!(
                "version: 1\nrules:\n  - name: t\n    locations: [{loc}]\n    conditions: [{{ name: {{ regex: '(' }} }}]\n    actions: [trash]\n"
            ),
        ),
        (
            "trash 不在最后",
            format!(
                "version: 1\nrules:\n  - name: t\n    locations: [{loc}]\n    actions: [trash, {{ move: /tmp }}]\n"
            ),
        ),
        (
            "模板",
            format!(
                "version: 1\nrules:\n  - name: t\n    locations: [{loc}]\n    actions: [{{ move: '/tmp/{{oops}}' }}]\n"
            ),
        ),
        (
            "根目录",
            "version: 1\nrules:\n  - name: t\n    locations: ['/']\n    actions: [trash]\n"
                .to_string(),
        ),
        (
            "相对位置",
            "version: 1\nrules:\n  - name: t\n    locations: ['Downloads']\n    actions: [trash]\n"
                .to_string(),
        ),
        (
            "大小单位",
            format!(
                "version: 1\nrules:\n  - name: t\n    locations: [{loc}]\n    conditions: [{{ size: {{ gt: 5XB }} }}]\n    actions: [trash]\n"
            ),
        ),
        (
            "重名",
            format!(
                "version: 1\nrules:\n  - {{ name: t, locations: [{loc}], actions: [trash] }}\n  - {{ name: t, locations: [{loc}], actions: [trash] }}\n"
            ),
        ),
    ];
    for (what, yaml) in cases {
        assert!(
            RuleSet::from_yaml(&yaml).is_err(),
            "应当拒绝：{what}\n{yaml}"
        );
    }
}

#[test]
fn example_rules_file_is_valid() {
    let text = include_str!("../../../examples/rules.yaml");
    let rs = RuleSet::from_yaml(text).unwrap();
    assert_eq!(rs.rules.len(), 5);
    assert_eq!(rs.folders.len(), 2);
    assert!(!rs.get("清理旧的小压缩包").unwrap().enabled);
    // 每个文件夹使用各自的规则与顺序
    let invoice = rs.index_of("发票归档").unwrap();
    assert_eq!(rs.folders_using(invoice).count(), 2);
    assert_eq!(
        rs.folders_using(rs.index_of("截图改名归档").unwrap())
            .count(),
        1
    );
}

#[test]
fn executing_selected_groups_keeps_chains_together() {
    let e = env();
    let a = touch(&e.inbox, "IMG_1.jpg", "1");
    let b = touch(&e.inbox, "IMG_2.jpg", "2");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: t\n    locations: [{}]\n    actions:\n      - rename: 'photo_{{name}}'\n      - move: {}\n",
        q(&e.inbox),
        q(&e.out)
    ));
    let plan = build_plan(&rs, None).unwrap();
    // 每个文件两步（改名 + 移动），同一个文件的两步属于同一分组
    let groups: Vec<usize> = plan.ops.iter().map(|o| o.group).collect();
    assert_eq!(groups, [0, 0, 1, 1]);
    assert_eq!(plan.group_count(), 2);

    let only_second = plan.select_groups(&[1].into_iter().collect());
    assert_eq!(only_second.ops.len(), 2);
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&only_second, &journal, "test").unwrap();
    assert_eq!(report.done, 2);
    assert!(a.exists(), "未勾选的文件不应被处理");
    assert!(!b.exists());
    assert!(e.out.join("photo_IMG_2.jpg").exists());
}

#[test]
fn rules_file_with_utf8_bom_loads() {
    let e = env();
    let path = e.inbox.join("rules.yaml");
    let yaml = format!(
        "\u{feff}version: 1\nrules:\n  - name: t\n    locations: [{}]\n    actions: [trash]\n",
        q(&e.inbox)
    );
    fs::write(&path, yaml).unwrap();
    assert_eq!(RuleSet::load(&path).unwrap().rules.len(), 1);
}

fn recursive_pdf_rules(e: &Env, extra_settings: &str) -> RuleSet {
    rules(&format!(
        r#"
version: 1
settings:
  {extra_settings}
rules:
  - name: pdf
    locations: [{inbox}]
    recursive: true
    conditions:
      - extension: pdf
    actions:
      - move: {dest}
"#,
        inbox = q(&e.inbox),
        dest = q(&e.out),
    ))
}

#[test]
fn folders_emptied_by_tidying_are_removed_and_restored_on_undo() {
    let e = env();
    let deep = touch(&e.inbox, "资料/2024/报告.pdf", "pdf");
    touch(&e.inbox, "资料/2024/.DS_Store", "junk");
    let other = touch(&e.inbox, "资料/备注.txt", "keep");
    let lonely = touch(&e.inbox, "单独/说明.pdf", "pdf");

    let journal = Journal::open_in_memory().unwrap();
    let plan = build_plan(&recursive_pdf_rules(&e, "auto: off"), None).unwrap();
    let report = execute(&plan, &journal, "test").unwrap();
    assert_eq!(report.done, 2);

    // 变空的文件夹被删除（只剩 .DS_Store 也算空），有其他文件的保留，整理范围本身保留
    assert!(!e.inbox.join("资料/2024").exists());
    assert!(!e.inbox.join("单独").exists());
    assert!(other.exists());
    assert!(e.inbox.is_dir());
    assert_eq!(report.removed_dirs.len(), 2);

    // 撤销时文件夹会重建
    undo_batch(&journal, report.batch_id).unwrap();
    assert!(deep.exists());
    assert!(lonely.exists());
}

#[test]
fn keep_empty_folders_setting() {
    let e = env();
    touch(&e.inbox, "单独/说明.pdf", "pdf");
    let journal = Journal::open_in_memory().unwrap();
    let plan = build_plan(&recursive_pdf_rules(&e, "keep_empty_folders: true"), None).unwrap();
    let report = execute(&plan, &journal, "test").unwrap();
    assert_eq!(report.done, 1);
    assert!(e.inbox.join("单独").is_dir());
    assert!(report.removed_dirs.is_empty());
}

#[test]
fn compress_extract_and_shortcut_then_undo() {
    let e = env();
    let doc = touch(&e.inbox, "报告.txt", "季度报告内容");
    let rs = rules(&format!(
        r#"
version: 2
folders:
  - path: {inbox}
    rules: [打包, 解压]
rules:
  - name: 打包
    when: {{ ext: txt }}
    actions:
      - compress: "{{stem}}.zip"
      - shortcut: {links}
  - name: 解压
    when: {{ ext: zip }}
    actions:
      - extract: "{{stem}}/"
"#,
        inbox = q(&e.inbox),
        links = q(&e.out),
    ));
    let journal = Journal::open_in_memory().unwrap();

    // 第一轮：压缩并创建快捷方式，原文件留在原处
    let report = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(report.done, 2, "{report:?}");
    let zip = e.inbox.join("报告.zip");
    assert!(doc.exists() && zip.exists());
    let link = if cfg!(windows) {
        e.out.join("报告.txt.lnk")
    } else {
        e.out.join("报告.txt")
    };
    assert!(link.symlink_metadata().is_ok());
    #[cfg(unix)]
    assert_eq!(fs::read_link(&link).unwrap(), doc);

    // 第二轮：解压刚生成的 zip 到同名文件夹
    let second = execute(&build_plan(&rs, Some("解压")).unwrap(), &journal, "test").unwrap();
    assert_eq!(second.done, 1, "{second:?}");
    assert_eq!(
        fs::read_to_string(e.inbox.join("报告/报告.txt")).unwrap(),
        "季度报告内容"
    );

    // 撤销第一轮：快捷方式删除、压缩包进回收站（测试里只检查快捷方式，避免真的往回收站放文件）
    let preview = foldo_core::preview_undo(&journal, report.batch_id).unwrap();
    assert!(
        preview
            .iter()
            .all(|c| c.outcome == foldo_core::UndoOutcome::Restore)
    );
}

#[test]
fn extract_rejects_unsupported_formats() {
    let e = env();
    touch(&e.inbox, "光盘.iso", "x");
    let rs = rules(&format!(
        "version: 2\nfolders:\n  - path: {}\n    rules: [解压]\nrules:\n  - name: 解压\n    actions: [{{ extract: '{{stem}}/' }}]\n",
        q(&e.inbox)
    ));
    let plan = build_plan(&rs, None).unwrap();
    assert!(plan.ops.is_empty());
    assert!(plan.skipped.iter().any(|s| s.reason.contains("zip")));
}

#[test]
fn keep_source_actions_are_not_repeated() {
    let e = env();
    touch(&e.inbox, "a.txt", "aaa");
    let rs = rules(&format!(
        "version: 2\nfolders:\n  - path: {inbox}\n    rules: [备份]\nrules:\n  - name: 备份\n    when: {{ ext: txt }}\n    actions: [{{ copy: {out} }}, {{ compress: '{{stem}}.zip' }}]\n",
        inbox = q(&e.inbox),
        out = q(&e.out),
    ));
    let journal = Journal::open_in_memory().unwrap();
    let first = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(first.done, 2);
    // 再扫描一次：副本和压缩包都已经在了，不再重复生成
    let again = build_plan(&rs, None).unwrap();
    assert!(again.ops.is_empty(), "{:?}", again.ops);
}

/// 在 `dir` 下生成一个含 `内容/说明.txt` 的压缩包，返回路径。
fn make_archive(dir: &Path, name: &str) -> PathBuf {
    let src = dir.join("待压缩");
    touch(&src, "内容/说明.txt", "压缩包里的文字");
    let out = dir.join(name);
    if name.ends_with(".7z") {
        sevenz_rust2::compress_to_path(&src, &out).unwrap();
    } else {
        let file = fs::File::create(&out).unwrap();
        let mut tar = tar::Builder::new(flate2::write::GzEncoder::new(
            file,
            flate2::Compression::default(),
        ));
        tar.append_dir_all(".", &src).unwrap();
        tar.into_inner().unwrap().finish().unwrap();
    }
    fs::remove_dir_all(&src).unwrap();
    set_mtime(&out, MAY_2024);
    out
}

#[test]
fn extract_7z_and_tar_gz() {
    let e = env();
    make_archive(&e.inbox, "资料.7z");
    make_archive(&e.inbox, "备份.tar.gz");
    let rs = rules(&format!(
        "version: 2\nfolders:\n  - path: {}\n    rules: [解压]\nrules:\n  - name: 解压\n    actions: [{{ extract: '{{stem}}/' }}]\n",
        q(&e.inbox)
    ));
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(report.done, 2, "{report:?}");
    assert_eq!(
        fs::read_to_string(e.inbox.join("资料/内容/说明.txt")).unwrap(),
        "压缩包里的文字"
    );
    // tar.gz 的主名是“备份.tar”
    assert_eq!(
        fs::read_to_string(e.inbox.join("备份.tar/内容/说明.txt")).unwrap(),
        "压缩包里的文字"
    );
}

#[test]
fn extract_never_writes_outside_the_target_folder() {
    let e = env();
    // 手工写一个含 ../逃逸.txt 的 tar（正常工具不会生成这种路径）
    let path = e.inbox.join("危险.tar");
    let mut builder = tar::Builder::new(fs::File::create(&path).unwrap());
    let body = b"evil";
    let mut header = tar::Header::new_old();
    header.as_old_mut().name[..12].copy_from_slice(b"../evil.txt\0");
    header.set_size(body.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    builder.append(&header, &body[..]).unwrap();
    builder.into_inner().unwrap();
    set_mtime(&path, MAY_2024);

    let rs = rules(&format!(
        "version: 2\nfolders:\n  - path: {}\n    rules: [解压]\nrules:\n  - name: 解压\n    actions: [{{ extract: '{{stem}}/' }}]\n",
        q(&e.inbox)
    ));
    let journal = Journal::open_in_memory().unwrap();
    execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert!(!e.inbox.join("evil.txt").exists());
    assert!(!e.inbox.parent().unwrap().join("evil.txt").exists());
}
