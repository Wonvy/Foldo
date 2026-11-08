//! 给用户看的提示文字的多语言：简体中文写在代码里作为原文，其他语言在 `locales/*.json` 里按原文查找译文。
//!
//! 用 [`t!`](crate::t) 代替 `format!`：`t!("最近 {span} 内新增或修改", span = s)`。
//! 找不到译文时用原文。语言由桌面端或命令行在启动时设置，默认简体中文。

use std::collections::HashMap;
use std::fmt::Display;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicU8, Ordering};

/// 支持的语言：0 简体中文（原文）、1 English、2 繁體中文、3 日本語。
static LANG: AtomicU8 = AtomicU8::new(0);

const CODES: [&str; 4] = ["zh-CN", "en", "zh-TW", "ja"];

static DICTS: LazyLock<[HashMap<String, String>; 3]> = LazyLock::new(|| {
    let parse = |s: &str| serde_json::from_str::<HashMap<String, String>>(s).unwrap_or_default();
    [
        parse(include_str!("../locales/en.json")),
        parse(include_str!("../locales/zh-TW.json")),
        parse(include_str!("../locales/ja.json")),
    ]
});

/// 设置语言：`zh-CN`、`en`、`zh-TW`、`ja`，也接受 `en_US.UTF-8`、`zh_Hant` 这类系统写法。
/// 不认识的语言用 English。
pub fn set_locale(code: &str) {
    LANG.store(parse_locale(code), Ordering::Relaxed);
}

fn parse_locale(code: &str) -> u8 {
    let c = code.to_lowercase().replace('_', "-");
    if c.starts_with("zh") {
        if ["hant", "tw", "hk", "mo"].iter().any(|x| c.contains(x)) {
            2
        } else {
            0
        }
    } else if c.starts_with("ja") {
        3
    } else {
        1
    }
}

/// 当前语言代码。
pub fn locale() -> &'static str {
    CODES[LANG.load(Ordering::Relaxed) as usize]
}

/// 按原文查找当前语言的译文。
pub fn tr(zh: &str) -> String {
    match LANG.load(Ordering::Relaxed) {
        0 => zh.to_string(),
        n => DICTS[n as usize - 1]
            .get(zh)
            .cloned()
            .unwrap_or_else(|| zh.to_string()),
    }
}

/// 把 `{name}` 换成对应的值。
pub fn fill(template: String, args: &[(&str, &dyn Display)]) -> String {
    let mut s = template;
    for (k, v) in args {
        s = s.replace(&format!("{{{k}}}"), &v.to_string());
    }
    s
}

/// 多语言版的 `format!`：`t!("找不到规则「{name}」", name = n)`。只支持具名占位符。
#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $crate::i18n::tr($s)
    };
    ($s:literal, $($k:ident = $v:expr),+ $(,)?) => {
        $crate::i18n::fill(
            $crate::i18n::tr($s),
            &[$((stringify!($k), &$v as &dyn ::std::fmt::Display)),+],
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locale_codes() {
        assert_eq!(parse_locale("zh-CN"), 0);
        assert_eq!(parse_locale("zh_Hans"), 0);
        assert_eq!(parse_locale("zh-TW"), 2);
        assert_eq!(parse_locale("zh_Hant_HK"), 2);
        assert_eq!(parse_locale("ja-JP"), 3);
        assert_eq!(parse_locale("en_US.UTF-8"), 1);
        assert_eq!(parse_locale("fr"), 1);
    }

    #[test]
    fn fill_named_placeholders() {
        let s = fill(
            "最近 {span} 内，共 {n} 个".into(),
            &[("span", &"10 分钟"), ("n", &3)],
        );
        assert_eq!(s, "最近 10 分钟 内，共 3 个");
    }
}
