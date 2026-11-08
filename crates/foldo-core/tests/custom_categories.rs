use foldo_core::meta::FileCtx;
use foldo_core::{RuleSet, build_plan, build_plan_for_paths, parse_rule_file};

#[test]
fn appinstaller_is_recognized_after_saving_the_custom_extension() {
    let tmp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let path = tmp.path().join("Shapr3D.Package.appinstaller");
    std::fs::write(&path, b"untouched").unwrap();
    let yaml = "version: 2\nrules:\n  - name: 软件\n    conditions: [{category: 安装包}]\n    actions: [keep]\n";
    let before = RuleSet::from_yaml(yaml).unwrap();
    assert_eq!(
        build_plan_for_paths(&before, std::slice::from_ref(&path), chrono::Local::now()).matched,
        0
    );
    let mut config = parse_rule_file(yaml).unwrap();
    config
        .settings
        .category_extensions
        .insert("installer".into(), vec!["appinstaller".into()]);
    let after = RuleSet::from_yaml(&config.to_yaml().unwrap()).unwrap();
    assert_eq!(
        build_plan_for_paths(&after, std::slice::from_ref(&path), chrono::Local::now()).matched,
        1
    );
    assert_eq!(std::fs::read(&path).unwrap(), b"untouched");
}

#[test]
fn custom_categories_apply_to_scan_drop_and_metadata() {
    let tmp = tempfile::tempdir_in(std::env::current_dir().unwrap()).unwrap();
    let file = tmp.path().join("photo.JXL");
    std::fs::write(&file, b"test").unwrap();
    let jpg = tmp.path().join("photo.jpg");
    std::fs::write(&jpg, b"test").unwrap();
    let mut config = parse_rule_file("version: 2\nsettings:\n  category_extensions:\n    image: ['.JXL']\nfolders:\n  - path: inbox\n    rules: [images]\nrules:\n  - name: images\n    conditions: [{category: 图片}]\n    actions: [keep]\n").unwrap();
    config.folders[0].path = tmp.path().to_string_lossy().into_owned();
    let yaml = config.to_yaml().unwrap();
    let set = RuleSet::from_yaml(&yaml).unwrap();
    let scan = build_plan(&set, None).unwrap();
    assert_eq!(scan.matched, 1);
    let drop = build_plan_for_paths(&set, &[file.clone(), jpg.clone()], chrono::Local::now());
    assert_eq!(drop.matched, 1);
    let mut ctx = FileCtx::from_path(file).unwrap();
    ctx.set_categories(&set.categories);
    assert_eq!(ctx.category().unwrap().id, "image");
    let mut removed = FileCtx::from_path(jpg).unwrap();
    removed.set_categories(&set.categories);
    assert!(removed.category().is_none());
    assert_eq!(set.categories.get("txt").unwrap().id, "document");
    let defaults = RuleSet::from_yaml("version: 2\nrules: []").unwrap();
    assert!(defaults.categories.get("jxl").is_none());
    assert_eq!(defaults.categories.get("JPG").unwrap().id, "image");
}

#[test]
fn invalid_or_conflicting_extensions_are_rejected() {
    for overrides in [
        "image: ['*.jxl']",
        "image: ['tar.gz']",
        "image: ['']",
        "unknown: [abc]",
        "image: [pdf]",
        "image: [abc]\n    document: [ABC]",
    ] {
        let yaml =
            format!("version: 2\nsettings:\n  category_extensions:\n    {overrides}\nrules: []");
        assert!(RuleSet::from_yaml(&yaml).is_err(), "{yaml}");
    }
    assert!(RuleSet::from_yaml("version: 2\nsettings:\n  category_extensions:\n    image: [pdf]\n    document: []\nrules: []").is_ok());
}
