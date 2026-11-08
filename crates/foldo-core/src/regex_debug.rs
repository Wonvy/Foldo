//! The rule editor's regex tester shares the rule engine's compiler and flags.
use regex::{Regex, RegexBuilder};
use serde::Serialize;

pub(crate) fn compile(pattern: &str, case_sensitive: bool) -> Result<Regex, regex::Error> {
    RegexBuilder::new(pattern)
        .case_insensitive(!case_sensitive)
        .build()
}

#[derive(Debug, Serialize)]
pub struct RegexCapture {
    pub index: usize,
    pub name: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegexSample {
    pub text: String,
    /// UTF-16 offsets, so the frontend can safely highlight Unicode text.
    pub range: Option<(usize, usize)>,
    pub captures: Vec<RegexCapture>,
}

pub fn test(
    pattern: &str,
    samples: &str,
    case_sensitive: bool,
) -> Result<Vec<RegexSample>, String> {
    if pattern.len() > 4096 || samples.len() > 65536 || samples.lines().count() > 100 {
        return Err(crate::t!("正则最多 4096 字节，测试文本最多 100 行、64 KB"));
    }
    let re = compile(pattern, case_sensitive).map_err(|e| e.to_string())?;
    Ok(samples
        .lines()
        .map(|text| {
            let caps = re.captures(text);
            let range = caps.as_ref().and_then(|c| c.get(0)).map(|m| {
                (
                    text[..m.start()].encode_utf16().count(),
                    text[..m.end()].encode_utf16().count(),
                )
            });
            let captures = caps
                .map(|caps| {
                    re.capture_names()
                        .enumerate()
                        .skip(1)
                        .map(|(index, name)| RegexCapture {
                            index,
                            name: name.map(str::to_owned),
                            value: caps.get(index).map(|m| m.as_str().to_owned()),
                        })
                        .collect()
                })
                .unwrap_or_default();
            RegexSample {
                text: text.to_owned(),
                range,
                captures,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_prefix_examples_and_boundaries() {
        let pattern = include_str!("../../../examples/date-prefix.regex").trim();
        let samples = test(pattern, "20160203 xxx\n1231 xxx\n26-12-12 xxx\n26.11.8 xxx\n2026-1-8\n普通文件夹\n1312 xxx\n20261301 xxx\n26.2.32 xxx\n12345 xxx\n20160203abc", false).unwrap();
        assert!(samples[..5].iter().all(|s| s.range.is_some()));
        assert!(samples[5..].iter().all(|s| s.range.is_none()));
    }

    #[test]
    fn matches_engine_flags_and_unicode_offsets() {
        let result = test("(?P<word>abc)", "🙂中文ABC", false).unwrap();
        assert_eq!(result[0].range, Some((4, 7)));
        assert_eq!(result[0].captures[0].name.as_deref(), Some("word"));
        assert_eq!(result[0].captures[0].value.as_deref(), Some("ABC"));
        assert!(test("abc", "ABC", true).unwrap()[0].range.is_none());
        assert!(test("(?=a)", "a", false).is_err());
        assert!(test("a", &"a\n".repeat(101), false).is_err());
    }
}
