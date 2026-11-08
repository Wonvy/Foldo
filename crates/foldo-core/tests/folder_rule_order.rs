use foldo_core::{RuleSet, build_plan, build_plan_for_paths, parse_rule_file};

#[test]
fn folders_choose_their_own_first_matching_rule_and_persist_order() {
    let tmp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let a = tmp.path().join("a");
    let b = tmp.path().join("b");
    std::fs::create_dir_all(&a).unwrap();
    std::fs::create_dir_all(&b).unwrap();
    let first = a.join("sample.txt");
    let second = b.join("sample.txt");
    std::fs::write(&first, "a").unwrap();
    std::fs::write(&second, "b").unwrap();
    let mut config = parse_rule_file(
        "version: 2\nfolders:\n  - path: a\n    rules: [second, first]\n  - path: b\n    rules: [first, second]\nrules:\n  - name: first\n    actions: [{move: 'one/'}]\n  - name: second\n    actions: [{move: 'two/'}]\n",
    ).unwrap();
    config.folders[0].path = a.to_string_lossy().into_owned();
    config.folders[1].path = b.to_string_lossy().into_owned();
    let set = RuleSet::from_yaml(&config.to_yaml().unwrap()).unwrap();
    let plan = build_plan(&set, None).unwrap();
    assert_eq!(plan.ops.len(), 2);
    assert_eq!(
        plan.ops.iter().find(|op| op.src == first).unwrap().rule,
        "second"
    );
    assert_eq!(
        plan.ops.iter().find(|op| op.src == second).unwrap().rule,
        "first"
    );

    // 文件夹重新排序，不修改其他文件夹或全局的拖入文件匹配顺序。
    config.folders[0].rules.as_mut().unwrap().reverse();
    let set = RuleSet::from_yaml(&config.to_yaml().unwrap()).unwrap();
    assert!(
        build_plan(&set, None)
            .unwrap()
            .ops
            .iter()
            .all(|op| op.rule == "first")
    );
    let drop = build_plan_for_paths(&set, &[first], chrono::Local::now());
    assert_eq!(drop.ops[0].rule, "first");

    // 停用规则不再抢先匹配。
    config.rules[0].enabled = false;
    let set = RuleSet::from_file(config).unwrap();
    assert!(
        build_plan(&set, None)
            .unwrap()
            .ops
            .iter()
            .all(|op| op.rule == "second")
    );
}

#[test]
fn repeated_folder_and_rule_references_keep_first_occurrence_order() {
    let tmp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let mut config = parse_rule_file(
        "version: 2\nfolders:\n  - path: a\n    rules: [second, first, second]\n  - path: a\n    rules: [third, first]\nrules:\n  - name: first\n    actions: [keep]\n  - name: second\n    actions: [keep]\n  - name: third\n    actions: [keep]\n",
    ).unwrap();
    for folder in &mut config.folders {
        folder.path = tmp.path().to_string_lossy().into_owned();
    }
    let set = RuleSet::from_file(config).unwrap();
    assert_eq!(set.folders.len(), 1);
    assert_eq!(set.folders[0].rules, vec![1, 0, 2]);
}
