//! 整理范围、只按用户规则整理、保护与排除、拖入文件、修改目标、撤销预览、配置写回。

use filetime::FileTime;
use foldo_core::config::RuleFile;
use foldo_core::{
    Journal, OpKind, RuleSet, SkipKind, UndoOutcome, build_plan, build_plan_at,
    build_plan_for_paths, execute, parse_rule_file, preview_undo,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tempfile::TempDir;

fn touch(dir: &Path, rel: &str, content: &str) -> PathBuf {
    let p = dir.join(rel);
    fs::create_dir_all(p.parent().unwrap()).unwrap();
    fs::write(&p, content).unwrap();
    // 默认设为两天前，避开缓冲期
    let old = SystemTime::now() - Duration::from_secs(2 * 86_400);
    filetime::set_file_mtime(&p, FileTime::from_system_time(old)).unwrap();
    p
}

fn q(p: &Path) -> String {
    format!("'{}'", p.display())
}

fn setup() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let inbox = tmp.path().canonicalize().unwrap().join("inbox");
    fs::create_dir_all(&inbox).unwrap();
    (tmp, inbox)
}

fn rules(yaml: &str) -> RuleSet {
    RuleSet::from_yaml(yaml).unwrap_or_else(|e| panic!("配置解析失败: {e}\n{yaml}"))
}

/// 整理文件夹 + 一条“所有文件 → 整理/”的规则，用来观察排除逻辑。
fn catch_all(inbox: &Path, extra: &str) -> RuleSet {
    rules(&format!(
        "version: 1\nfolders:\n  - path: {}\n{extra}rules:\n  - name: 全部\n    actions: [{{ move: '整理/' }}]\n",
        q(inbox)
    ))
}

fn skip_kind(plan: &foldo_core::Plan, name: &str) -> Option<SkipKind> {
    plan.skipped
        .iter()
        .find(|s| s.path.file_name().unwrap() == name)
        .map(|s| s.kind)
}

fn dst_of(plan: &foldo_core::Plan, name: &str) -> Option<PathBuf> {
    plan.ops
        .iter()
        .find(|o| o.origin.file_name().unwrap() == name)
        .and_then(|o| o.dst.clone())
}

#[test]
fn only_user_rules_move_files() {
    let (_t, inbox) = setup();
    fs::create_dir_all(inbox.join("Pictures")).unwrap();
    touch(&inbox, "a.JPG", "");
    touch(&inbox, "b.pdf", "");
    touch(&inbox, "项目A/内部.jpg", "");

    // 只有整理文件夹、没有规则：什么都不动
    let rs = rules(&format!("version: 1\nfolders:\n  - path: {}\n", q(&inbox)));
    let plan = build_plan(&rs, None).unwrap();
    assert!(plan.ops.is_empty());
    assert_eq!(skip_kind(&plan, "a.JPG"), Some(SkipKind::NoMatch));
    assert_eq!(skip_kind(&plan, "b.pdf"), Some(SkipKind::NoMatch));
    // 已有文件夹整体保留，内部文件不拆
    assert_eq!(skip_kind(&plan, "项目A"), Some(SkipKind::Folder));

    // 用分类作为条件：目标写什么就是什么，不会自动映射到已有的 Pictures
    let rs = rules(&format!(
        "version: 1\nfolders:\n  - path: {}\nrules:\n  - name: 图片\n    conditions: [{{ category: 图片 }}]\n    actions: [{{ move: '图片/' }}]\n",
        q(&inbox)
    ));
    let plan = build_plan(&rs, None).unwrap();
    assert_eq!(dst_of(&plan, "a.JPG"), Some(inbox.join("图片/a.JPG")));
    let op = &plan.ops[0];
    assert_eq!(op.reason, "规则「图片」");
    assert_eq!(op.category.as_deref(), Some("图片"));
    assert_eq!(skip_kind(&plan, "b.pdf"), Some(SkipKind::NoMatch));
    assert!(dst_of(&plan, "内部.jpg").is_none());
}

#[test]
fn rule_order_locations_and_exceptions() {
    let (_t, inbox) = setup();
    touch(&inbox, "合同.pdf", "");
    touch(&inbox, "合同草稿.pdf", "");
    let rs = rules(&format!(
        r#"
version: 1
folders:
  - path: {inbox}
rules:
  - name: 合同
    conditions: [{{ name: {{ contains: 合同 }} }}]
    except: [{{ name: {{ contains: 草稿 }} }}]
    actions: [{{ move: '合同/' }}]
  - name: 文档
    conditions: [{{ category: 文档 }}]
    actions: [{{ move: '文档/' }}]
"#,
        inbox = q(&inbox)
    ));
    // 没写 locations 的规则作用于整理文件夹
    assert_eq!(rs.get("合同").unwrap().locations, vec![inbox.clone()]);
    let plan = build_plan(&rs, None).unwrap();
    assert_eq!(dst_of(&plan, "合同.pdf"), Some(inbox.join("合同/合同.pdf")));
    // 例外命中后不归第一条规则，由下一条规则处理
    assert_eq!(
        dst_of(&plan, "合同草稿.pdf"),
        Some(inbox.join("文档/合同草稿.pdf"))
    );
}

#[test]
fn protected_credential_shortcut_and_system_items_never_move() {
    let (_t, inbox) = setup();
    let keep = touch(&inbox, "保留.pdf", "");
    touch(&inbox, "授权.LIC", "");
    touch(&inbox, "server.pem", "");
    touch(&inbox, "id_ed25519", "");
    touch(&inbox, "私钥.key", "-----BEGIN PRIVATE KEY-----\n");
    touch(&inbox, "演示.key", "PK\u{3}\u{4}");
    touch(&inbox, "Chrome.lnk", "");
    touch(&inbox, ".DS_Store", "");
    touch(&inbox, "movie.mp4.crdownload", "");
    touch(&inbox, ".hidden.png", "");

    let rs = catch_all(
        &inbox,
        &format!("protect:\n  - {}\n  - '*.lic'\n", q(&keep)),
    );
    let plan = build_plan(&rs, None).unwrap();
    let expect = [
        ("保留.pdf", SkipKind::Protected),
        ("授权.LIC", SkipKind::Protected),
        ("server.pem", SkipKind::Credential),
        ("id_ed25519", SkipKind::Credential),
        ("私钥.key", SkipKind::Credential),
        ("Chrome.lnk", SkipKind::Shortcut),
        (".DS_Store", SkipKind::System),
        ("movie.mp4.crdownload", SkipKind::Partial),
        (".hidden.png", SkipKind::Hidden),
    ];
    for (name, kind) in expect {
        assert_eq!(skip_kind(&plan, name), Some(kind), "{name}");
        assert!(dst_of(&plan, name).is_none(), "{name} 不应出现在移动清单中");
    }
    // Keynote 演示文稿 .key 不是私钥，照常整理
    assert_eq!(dst_of(&plan, "演示.key"), Some(inbox.join("整理/演示.key")));
}

#[test]
fn buffer_period_skips_new_files() {
    let (_t, inbox) = setup();
    touch(&inbox, "old.pdf", "");
    fs::write(inbox.join("new.pdf"), "").unwrap(); // 刚刚创建
    let rs = catch_all(&inbox, "settings:\n  buffer: 1h\n");
    let plan = build_plan(&rs, None).unwrap();
    assert_eq!(skip_kind(&plan, "new.pdf"), Some(SkipKind::Buffer));
    // 缓冲期看“创建和修改中较新的那个”。Linux / Windows 上无法把创建时间改到过去，
    // 测试里的 old.pdf 创建时间仍是刚才，这时它也算新文件，跳过这条断言。
    let created_recently = fs::metadata(inbox.join("old.pdf"))
        .and_then(|m| m.created())
        .map(|c| c.elapsed().unwrap_or_default() < Duration::from_secs(3600))
        .unwrap_or(false);
    if !created_recently {
        assert!(dst_of(&plan, "old.pdf").is_some());
    }

    let rs = catch_all(&inbox, "settings:\n  buffer: 3d\n");
    let plan = build_plan_at(&rs, None, chrono::Local::now()).unwrap();
    assert_eq!(skip_kind(&plan, "old.pdf"), Some(SkipKind::Buffer));
}

#[test]
fn dropped_files_ignore_rule_locations() {
    let (_t, inbox) = setup();
    let elsewhere = inbox.parent().unwrap().join("elsewhere");
    let step = touch(&elsewhere, "旋钮工装.step", "");
    let other = touch(&elsewhere, "随便.pdf", "");
    fs::create_dir_all(elsewhere.join("一个文件夹")).unwrap();
    let rs = rules(&format!(
        r#"
version: 1
folders:
  - path: {inbox}
rules:
  - name: 模型
    conditions: [{{ category: 3D 模型 }}]
    actions: [{{ move: '模型/' }}]
"#,
        inbox = q(&inbox)
    ));
    let plan = build_plan_for_paths(
        &rs,
        &[step.clone(), other.clone(), elsewhere.join("一个文件夹")],
        chrono::Local::now(),
    );
    // 拖入的文件不受适用范围限制，相对路径以文件所在文件夹为基准
    assert_eq!(
        dst_of(&plan, "旋钮工装.step"),
        Some(elsewhere.join("模型/旋钮工装.step"))
    );
    assert_eq!(skip_kind(&plan, "随便.pdf"), Some(SkipKind::NoMatch));
    assert_eq!(skip_kind(&plan, "一个文件夹"), Some(SkipKind::Folder));
}

#[test]
fn retarget_group_replaces_chain_and_avoids_conflicts() {
    let (_t, inbox) = setup();
    touch(&inbox, "IMG_1.jpg", "");
    let target = inbox.parent().unwrap().join("精选");
    touch(&target, "IMG_1.jpg", "已有");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: t\n    locations: [{}]\n    actions: [{{ rename: 'x_{{name}}' }}, {{ move: '整理/' }}]\n",
        q(&inbox)
    ));
    let mut plan = build_plan(&rs, None).unwrap();
    assert_eq!(plan.ops.len(), 2);
    let dst = plan.retarget_group(0, &target).unwrap();
    assert_eq!(dst, target.join("IMG_1 (1).jpg"));
    assert_eq!(plan.ops.len(), 1);
    assert_eq!(plan.ops[0].kind, OpKind::Move);
    assert_eq!(plan.ops[0].reason, "手动指定位置");
}

#[test]
fn undo_preview_follows_chains_and_reports_blockers() {
    let (_t, inbox) = setup();
    touch(&inbox, "a.txt", "1");
    touch(&inbox, "b.txt", "2");
    let out = inbox.parent().unwrap().join("out");
    let rs = rules(&format!(
        "version: 1\nrules:\n  - name: t\n    locations: [{}]\n    actions: [{{ rename: 'r_{{name}}' }}, {{ move: {} }}]\n",
        q(&inbox),
        q(&out)
    ));
    let journal = Journal::open_in_memory().unwrap();
    let report = execute(&build_plan(&rs, None).unwrap(), &journal, "test").unwrap();
    assert_eq!(report.done, 4);

    // 用户之后把一个文件挪走了
    fs::remove_file(out.join("r_b.txt")).unwrap();
    let checks = preview_undo(&journal, report.batch_id).unwrap();
    assert_eq!(checks.len(), 4);
    let ok = checks
        .iter()
        .filter(|c| c.outcome == UndoOutcome::Restore)
        .count();
    let blocked: Vec<_> = checks
        .iter()
        .filter(|c| c.outcome == UndoOutcome::Blocked)
        .collect();
    // a 的“改名 + 移动”都能恢复；b 的移动无法恢复，改名也因此无法恢复
    assert_eq!(ok, 2);
    assert_eq!(blocked.len(), 2);
    assert!(blocked.iter().all(|c| c.reason.is_some()));
    // 预览不改动任何文件
    assert!(out.join("r_a.txt").exists());
}

#[test]
fn config_roundtrips_through_yaml_and_json() {
    let text = include_str!("../../../examples/rules.yaml");
    let file = parse_rule_file(text).unwrap();
    let yaml = file.to_yaml().unwrap();
    let again = parse_rule_file(&yaml).unwrap();
    assert_eq!(again.to_yaml().unwrap(), yaml, "写回再读取后应保持一致");
    RuleSet::from_file(again).unwrap();

    // 图形界面用 JSON 传递配置
    let json = serde_json::to_string(&file).unwrap();
    let from_json: RuleFile = serde_json::from_str(&json).unwrap();
    assert_eq!(from_json.to_yaml().unwrap(), yaml);
    assert!(yaml.contains("buffer: 10m"));
    assert!(yaml.contains("- trash"));
    assert!(yaml.contains("category: 图片"));
}

#[test]
fn keep_rule_leaves_matching_files_in_place() {
    let (_t, inbox) = setup();
    touch(&inbox, "装配.step", "");
    touch(&inbox, "照片.jpg", "");
    let rs = rules(&format!(
        "version: 1\nfolders:\n  - path: {}\nrules:\n  - name: 模型别动\n    conditions: [{{ extension: step }}]\n    actions: [keep]\n  - name: 其余\n    actions: [{{ move: '整理/' }}]\n",
        q(&inbox)
    ));
    let plan = build_plan(&rs, None).unwrap();
    assert_eq!(skip_kind(&plan, "装配.step"), Some(SkipKind::Protected));
    assert!(dst_of(&plan, "装配.step").is_none());
    assert!(dst_of(&plan, "照片.jpg").is_some());
    assert!(
        RuleSet::from_yaml(
            "version: 1\nrules:\n  - name: x\n    locations: ['/tmp']\n    actions: [keep, trash]\n"
        )
        .is_err()
    );
}

#[test]
fn old_category_switches_are_rejected_with_a_clear_error() {
    // 内置自动分类已移除：旧配置里的 categories 字段会报错，提示去掉
    let err =
        RuleSet::from_yaml("version: 1\nfolders:\n  - path: '/tmp/x'\n    categories: true\n")
            .unwrap_err()
            .to_string();
    assert!(err.contains("categories"), "{err}");
}

#[test]
fn auto_mode_holds_back_irreversible_actions() {
    let (_t, inbox) = setup();
    touch(&inbox, "a.pdf", "");
    touch(&inbox, "old.zip", "");
    let yaml = format!(
        "version: 1\nfolders:\n  - path: {}\nsettings:\n  auto: run\nrules:\n  - name: 压缩包\n    conditions: [{{ ext: zip }}]\n    actions: [trash]\n  - name: 文档\n    conditions: [{{ ext: pdf }}]\n    actions: [{{ move: '文档/' }}]\n",
        q(&inbox)
    );
    let rs = rules(&yaml);
    assert_eq!(rs.auto, foldo_core::config::AutoMode::Run);
    let plan = build_plan(&rs, None).unwrap();
    let (safe, held) = plan.without_irreversible();
    assert_eq!(held, 1);
    assert_eq!(safe.ops.len(), 1);
    assert_eq!(safe.ops[0].kind, OpKind::Move);
    // 写回 YAML 时保留设置
    let file = parse_rule_file(&yaml).unwrap();
    assert!(file.to_yaml().unwrap().contains("auto: run"));
}
