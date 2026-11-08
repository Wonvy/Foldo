//! 多语言完整性：界面和后台里每一段要翻译的文字，在 English、繁體中文、日本語里都要有译文，
//! 而且占位符（{0}、{name}…）一致。
//!
//! 设置环境变量 `FOLDO_I18N_DUMP=<目录>` 时，把需要翻译的原文列表写到该目录（ui-keys.json、rs-keys.json），方便补译文。

use foldo_core::fields::FIELDS;
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

const LANGS: [&str; 3] = ["en", "zh-TW", "ja"];

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sources(dir: &Path, exts: &[&str]) -> Vec<(PathBuf, String)> {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let p = e.path().to_string_lossy();
            !p.contains("/dev/")
                && !p.contains("/locales/")
                && !p.contains("/node_modules/")
                && !p.contains("/test/")
                && !p.contains(".test.")
        })
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|x| exts.contains(&x.to_string_lossy().as_ref()))
        })
        .map(|e| {
            (
                e.path().to_path_buf(),
                std::fs::read_to_string(e.path()).unwrap(),
            )
        })
        .collect()
}

/// 字符串字面量的内容（处理 \" \\ \n 等转义）。
fn unescape(lit: &str) -> String {
    serde_json::from_str(&format!("\"{lit}\"")).unwrap_or_else(|_| lit.to_string())
}

/// 界面需要的原文：代码里的 t("…")，加上后端提供、在界面上显示的字段名、说明、可选值、类型名、变量名等。
fn ui_keys() -> BTreeSet<String> {
    let re = Regex::new(r#"\bt\(\s*"((?:[^"\\]|\\.)*)""#).unwrap();
    let mut keys = BTreeSet::new();
    for (_, text) in sources(&repo().join("apps/desktop/src"), &["ts", "tsx"]) {
        for c in re.captures_iter(&text) {
            keys.insert(unescape(&c[1]));
        }
    }
    let cjk = Regex::new(r"[\u{3400}-\u{9fff}]").unwrap();
    let mut add = |s: &str| {
        if cjk.is_match(s) {
            keys.insert(s.to_string());
        }
    };
    for f in FIELDS.iter() {
        add(f.label);
        add(f.desc);
        add(f.group);
        if let Some(u) = f.unit {
            add(u);
        }
        for o in f.options {
            add(o);
        }
    }
    for c in foldo_core::categories::CATEGORIES {
        add(c.name);
    }
    for v in foldo_core::template::SPECIAL_VARS {
        add(v.label);
        add(v.desc);
        add(v.group);
    }
    // 后端返回、界面翻译显示的固定文字
    for s in [
        "是",
        "否",
        "文件夹",
        "移动到",
        "复制到",
        "解压到",
        "压缩为",
        "创建快捷方式到",
        "改名为",
        "移到回收站",
        "保持原位",
    ] {
        add(s);
    }
    keys
}

/// 后台和命令行需要的原文：t!("…")、tr("…")，以及托盘菜单和跳过原因的标签。
fn rs_keys() -> BTreeSet<String> {
    let re = Regex::new(r#"\b(?:t!|tr)\(\s*"((?:[^"\\]|\\.)*)""#).unwrap();
    let lit = Regex::new(r#""((?:[^"\\]|\\.)*[\u{3400}-\u{9fff}](?:[^"\\]|\\.)*)""#).unwrap();
    let mut keys = BTreeSet::new();
    for dir in [
        "crates/foldo-core/src",
        "crates/foldo-cli/src",
        "apps/desktop/src-tauri/src",
    ] {
        for (path, text) in sources(&repo().join(dir), &["rs"]) {
            let body = text.split("#[cfg(test)]").next().unwrap_or("");
            for c in re.captures_iter(body) {
                keys.insert(unescape(&c[1]));
            }
            // 托盘菜单的文字都要翻译
            if path.ends_with("tray.rs") {
                for line in body.lines().filter(|l| !l.trim_start().starts_with("//")) {
                    for c in lit.captures_iter(line) {
                        keys.insert(unescape(&c[1]));
                    }
                }
            }
        }
    }
    // 命令行帮助：i18n-help-start 和 i18n-help-end 之间的 /// 说明和 about = "…"
    let cli = std::fs::read_to_string(repo().join("crates/foldo-cli/src/main.rs")).unwrap();
    if let (Some(a), Some(b)) = (cli.find("i18n-help-start"), cli.find("i18n-help-end")) {
        let about = Regex::new(r#"about = "((?:[^"\\]|\\.)*)""#).unwrap();
        for line in cli[a..b].lines() {
            let l = line.trim();
            if let Some(doc) = l.strip_prefix("///") {
                keys.insert(doc.trim().to_string());
            }
            for c in about.captures_iter(l) {
                keys.insert(unescape(&c[1]));
            }
        }
    }
    use foldo_core::SkipKind::*;
    for k in [
        Protected, Credential, Shortcut, System, Hidden, Partial, Buffer, InUse, Folder, NoMatch,
        InPlace, Conflict, Invalid,
    ] {
        keys.insert(k.label().to_string());
    }
    keys
}

fn placeholders(s: &str) -> BTreeSet<String> {
    Regex::new(r"\{[A-Za-z0-9_]+\}")
        .unwrap()
        .find_iter(s)
        .map(|m| m.as_str().to_string())
        .collect()
}

fn check(dir: &str, keys: &BTreeSet<String>) -> Vec<String> {
    let mut problems = Vec::new();
    for lang in LANGS {
        let path = repo().join(dir).join(format!("{lang}.json"));
        let dict: BTreeMap<String, String> =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        for k in keys {
            match dict.get(k) {
                None => problems.push(format!("{dir}/{lang}.json 缺少：{k}")),
                Some(v) if placeholders(v) != placeholders(k) => {
                    problems.push(format!("{dir}/{lang}.json 占位符不一致：{k} → {v}"))
                }
                _ => {}
            }
        }
    }
    problems
}

#[test]
fn every_text_is_translated() {
    let ui = ui_keys();
    let rs = rs_keys();
    if let Ok(dir) = std::env::var("FOLDO_I18N_DUMP") {
        let dir = PathBuf::from(dir);
        std::fs::write(
            dir.join("ui-keys.json"),
            serde_json::to_string_pretty(&ui).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.join("rs-keys.json"),
            serde_json::to_string_pretty(&rs).unwrap(),
        )
        .unwrap();
        return;
    }
    // 界面和后台共用一份译文
    let all: BTreeSet<String> = ui.union(&rs).cloned().collect();
    let problems = check("locales", &all);
    assert!(
        problems.is_empty(),
        "{} 处翻译缺失或有误（前 30 条）：\n{}",
        problems.len(),
        problems
            .iter()
            .take(30)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
