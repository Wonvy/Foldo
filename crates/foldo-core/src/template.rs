//! 路径模板：`{dir.pictures}/{exif.taken ?? modified:%Y}/{stem|slug}.{ext|lower}`
//!
//! 语法：`{变量[:时间格式] [?? 备选 ...] [|过滤器 ...]}`，用 `{{` / `}}` 输出字面量花括号。
//!
//! - 变量：字段注册表里的所有字段（`name` `ext` `size` `image.width` `exif.model`……），以及
//!   `now`、`match.N` / `match.组名`（正则捕获，旧写法 `regex.N` 也可以）、
//!   `dir.home|desktop|documents|downloads|pictures|music|videos`、
//!   `system.user|host|os`、`env.任意环境变量`
//! - 时间格式：strftime，如 `{modified:%Y-%m}`；默认 `%Y-%m-%d`。备选里只有最后一个写了格式时，前面的也沿用它
//! - 备选：`{exif.taken ?? created ?? modified:%Y}`，也可以用引号写死：`{exif.model ?? "未知相机"}`
//! - 过滤器：`lower` `upper` `title` `slug` `trim` `pad(3)` `slice(0,8)` `replace("正则","替换")` `default("值")`
//!
//! 文件属性变量的值会去掉 `: * ? " < > |` 等在 Windows/macOS 上不合法的字符，保证三个平台行为一致；
//! `dir.*`、`env.*`、`path` 是路径，原样输出。

use crate::fields::{self, FieldDef, FieldType, Value};
use crate::fileinfo::split_name;
use crate::meta::FileCtx;
use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Local};
use regex::Regex;
use serde::Serialize;
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct Template {
    source: String,
    parts: Vec<Part>,
}

#[derive(Debug, Clone)]
enum Part {
    Lit(String),
    Expr {
        alts: Vec<Alt>,
        filters: Vec<Filter>,
    },
}

#[derive(Debug, Clone)]
enum Alt {
    Lit(String),
    Var { var: Var, fmt: Option<String> },
}

#[derive(Debug, Clone)]
enum Var {
    Field(&'static FieldDef),
    /// 来自文件“当前”路径（前面的动作可能改过名）
    Cur(CurVar),
    Now,
    Match(String),
    Env(String),
    Dir(&'static str),
    System(&'static str),
}

#[derive(Debug, Clone, Copy)]
enum CurVar {
    Name,
    Stem,
    Ext,
    Parent,
}

#[derive(Debug, Clone)]
enum Filter {
    Lower,
    Upper,
    Title,
    Slug,
    Trim,
    Pad(usize),
    Slice(usize, Option<usize>),
    Replace(Regex, String),
    Default(String),
}

/// 渲染模板需要的信息。`cur` 是文件当前（可能已被前面的动作改过名）的路径。
pub struct TemplateCtx<'a> {
    pub cur: &'a Path,
    pub file: &'a FileCtx,
    pub captures: &'a BTreeMap<String, String>,
    pub now: DateTime<Local>,
}

const DEFAULT_DATE_FMT: &str = "%Y-%m-%d";

const DIRS: &[&str] = &[
    "home",
    "desktop",
    "documents",
    "downloads",
    "pictures",
    "music",
    "videos",
];
const SYSTEM: &[&str] = &["user", "host", "os"];

/// 字段之外的变量（供界面的“插入变量”菜单和文档使用）。
#[derive(Debug, Serialize)]
pub struct VarDef {
    pub key: &'static str,
    pub label: &'static str,
    pub group: &'static str,
    pub desc: &'static str,
}

pub static SPECIAL_VARS: &[VarDef] = &[
    VarDef {
        key: "now:%Y-%m-%d",
        label: "当前时间",
        group: "时间",
        desc: "执行整理时的时间，可改格式",
    },
    VarDef {
        key: "match.1",
        label: "正则捕获 1",
        group: "正则",
        desc: "条件中正则的第 1 个分组；命名分组用 match.名字",
    },
    VarDef {
        key: "dir.home",
        label: "用户主目录",
        group: "系统文件夹",
        desc: "如 /Users/me 或 C:\\Users\\me",
    },
    VarDef {
        key: "dir.desktop",
        label: "桌面",
        group: "系统文件夹",
        desc: "",
    },
    VarDef {
        key: "dir.documents",
        label: "文稿",
        group: "系统文件夹",
        desc: "",
    },
    VarDef {
        key: "dir.downloads",
        label: "下载",
        group: "系统文件夹",
        desc: "",
    },
    VarDef {
        key: "dir.pictures",
        label: "图片",
        group: "系统文件夹",
        desc: "",
    },
    VarDef {
        key: "dir.music",
        label: "音乐",
        group: "系统文件夹",
        desc: "",
    },
    VarDef {
        key: "dir.videos",
        label: "影片",
        group: "系统文件夹",
        desc: "",
    },
    VarDef {
        key: "system.user",
        label: "用户名",
        group: "系统",
        desc: "",
    },
    VarDef {
        key: "system.host",
        label: "电脑名",
        group: "系统",
        desc: "",
    },
    VarDef {
        key: "system.os",
        label: "操作系统",
        group: "系统",
        desc: "macOS / Windows / Linux",
    },
    VarDef {
        key: "env.HOME",
        label: "环境变量",
        group: "系统",
        desc: "env. 后面写任意环境变量名，如 env.USERPROFILE",
    },
];

impl Template {
    pub fn parse(src: &str) -> Result<Self, String> {
        let mut parts = Vec::new();
        let mut lit = String::new();
        let mut chars = src.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    lit.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    lit.push('}');
                }
                '{' => {
                    // 读到匹配的 }，引号里的 } 不算
                    let mut expr = String::new();
                    let mut quote: Option<char> = None;
                    let mut closed = false;
                    for c in chars.by_ref() {
                        match (quote, c) {
                            (None, '}') => {
                                closed = true;
                                break;
                            }
                            (None, '"' | '\'') => quote = Some(c),
                            (Some(q), c) if c == q => quote = None,
                            _ => {}
                        }
                        expr.push(c);
                    }
                    if !closed {
                        return Err(format!("模板「{src}」中有未闭合的 {{"));
                    }
                    if !lit.is_empty() {
                        parts.push(Part::Lit(std::mem::take(&mut lit)));
                    }
                    parts.push(parse_expr(&expr).map_err(|e| format!("模板「{src}」: {e}"))?);
                }
                '}' => return Err(format!("模板「{src}」中有多余的 }}，字面量请写成 }}}}")),
                c => lit.push(c),
            }
        }
        if !lit.is_empty() {
            parts.push(Part::Lit(lit));
        }
        Ok(Self {
            source: src.to_string(),
            parts,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    /// 是否包含变量（否则就是普通文本）。
    pub fn has_vars(&self) -> bool {
        self.parts.iter().any(|p| matches!(p, Part::Expr { .. }))
    }

    /// 渲染为路径片段：文件属性变量会去掉不合法字符。
    pub fn render(&self, ctx: &TemplateCtx) -> Result<String, String> {
        self.render_with(ctx, true)
    }

    /// 原样渲染（用于条件里的比较值）。
    pub fn render_raw(&self, ctx: &TemplateCtx) -> Result<String, String> {
        self.render_with(ctx, false)
    }

    fn render_with(&self, ctx: &TemplateCtx, sanitize_values: bool) -> Result<String, String> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                Part::Lit(s) => out.push_str(s),
                Part::Expr { alts, filters } => {
                    let (mut v, raw) = match eval_alts(alts, ctx)? {
                        Some(hit) => hit,
                        None => match filters.iter().find_map(|f| match f {
                            Filter::Default(d) => Some(d.clone()),
                            _ => None,
                        }) {
                            Some(d) => (d, false),
                            None => return Err(format!("无法获取「{}」", describe(alts))),
                        },
                    };
                    for f in filters {
                        v = apply_filter(f, &v);
                    }
                    out.push_str(&if sanitize_values && !raw {
                        sanitize(&v)
                    } else {
                        v
                    });
                }
            }
        }
        Ok(out)
    }
}

fn describe(alts: &[Alt]) -> String {
    alts.iter()
        .map(|a| match a {
            Alt::Lit(s) => format!("\"{s}\""),
            Alt::Var { var, .. } => match var {
                Var::Field(f) => f.key.to_string(),
                Var::Cur(c) => format!("{c:?}").to_lowercase(),
                Var::Now => "now".into(),
                Var::Match(k) => format!("match.{k}"),
                Var::Env(k) => format!("env.{k}"),
                Var::Dir(k) => format!("dir.{k}"),
                Var::System(k) => format!("system.{k}"),
            },
        })
        .collect::<Vec<_>>()
        .join(" ?? ")
}

/// 在引号和括号之外按分隔符切分。
fn split_top(s: &str, sep: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0i32;
    let chars: Vec<char> = s.chars().collect();
    let sep: Vec<char> = sep.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == '\\' && i + 1 < chars.len() {
                    cur.push(c);
                    cur.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == '(' {
                    depth += 1;
                } else if c == ')' {
                    depth -= 1;
                } else if depth == 0 && chars[i..].starts_with(&sep) {
                    out.push(std::mem::take(&mut cur));
                    i += sep.len();
                    continue;
                }
            }
        }
        cur.push(c);
        i += 1;
    }
    out.push(cur);
    out
}

fn unquote(s: &str) -> Option<String> {
    let s = s.trim();
    let q = s.chars().next()?;
    if (q == '"' || q == '\'') && s.len() >= 2 && s.ends_with(q) {
        Some(s[1..s.len() - 1].replace(&format!("\\{q}"), &q.to_string()))
    } else {
        None
    }
}

fn parse_expr(expr: &str) -> Result<Part, String> {
    let segs = split_top(expr, "|");
    let head = segs[0].trim();
    if head.is_empty() {
        return Err("空的变量 {}".into());
    }
    let mut alts = Vec::new();
    for raw in split_top(head, "??") {
        let raw = raw.trim();
        if let Some(lit) = unquote(raw) {
            alts.push(Alt::Lit(lit));
            continue;
        }
        let (name, fmt) = match raw.split_once(':') {
            Some((n, f)) => (n.trim(), Some(f.to_string())),
            None => (raw, None),
        };
        let var = parse_var(name)?;
        if let Some(f) = &fmt {
            let is_date =
                matches!(var, Var::Now) || matches!(var, Var::Field(d) if d.ty == FieldType::Date);
            if !is_date {
                return Err(format!("变量「{name}」不是时间，不能写格式"));
            }
            if StrftimeItems::new(f).any(|i| matches!(i, Item::Error)) {
                return Err(format!("时间格式「{f}」无效"));
            }
        }
        alts.push(Alt::Var { var, fmt });
    }
    // 只有后面的备选写了时间格式时，前面的时间变量沿用它
    let last_fmt = alts.iter().rev().find_map(|a| match a {
        Alt::Var { fmt: Some(f), .. } => Some(f.clone()),
        _ => None,
    });
    for a in &mut alts {
        if let Alt::Var { var, fmt } = a
            && fmt.is_none()
            && (matches!(var, Var::Now) || matches!(var, Var::Field(d) if d.ty == FieldType::Date))
        {
            *fmt = last_fmt.clone();
        }
    }
    let filters = segs[1..]
        .iter()
        .map(|s| parse_filter(s.trim()))
        .collect::<Result<_, _>>()?;
    Ok(Part::Expr { alts, filters })
}

fn parse_var(name: &str) -> Result<Var, String> {
    Ok(match name {
        "now" => Var::Now,
        "name" => Var::Cur(CurVar::Name),
        "stem" => Var::Cur(CurVar::Stem),
        "ext" => Var::Cur(CurVar::Ext),
        "parent" => Var::Cur(CurVar::Parent),
        n if n.starts_with("match.") || n.starts_with("regex.") => {
            let k = &n[6..];
            if k.is_empty() {
                return Err(format!("「{n}」缺少分组名或序号"));
            }
            Var::Match(k.to_string())
        }
        n if n.starts_with("env.") && n.len() > 4 => Var::Env(n[4..].to_string()),
        n if n.starts_with("dir.") => {
            Var::Dir(DIRS.iter().find(|d| **d == &n[4..]).ok_or_else(|| {
                format!("未知系统文件夹「{n}」，可用：{}", DIRS.map_prefix("dir."))
            })?)
        }
        n if n.starts_with("system.") => {
            Var::System(SYSTEM.iter().find(|d| **d == &n[7..]).ok_or_else(|| {
                format!(
                    "未知系统变量「{n}」，可用：{}",
                    SYSTEM.map_prefix("system.")
                )
            })?)
        }
        n => Var::Field(fields::field(n).ok_or_else(|| format!("未知变量「{n}」"))?),
    })
}

trait MapPrefix {
    fn map_prefix(&self, p: &str) -> String;
}
impl MapPrefix for &[&str] {
    fn map_prefix(&self, p: &str) -> String {
        self.iter()
            .map(|x| format!("{p}{x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn parse_filter(s: &str) -> Result<Filter, String> {
    let (name, args) = match s.split_once('(') {
        Some((n, rest)) => {
            let inner = rest
                .strip_suffix(')')
                .ok_or_else(|| format!("过滤器「{s}」缺少右括号"))?;
            (
                n.trim(),
                split_top(inner, ",")
                    .into_iter()
                    .map(|a| a.trim().to_string())
                    .collect::<Vec<_>>(),
            )
        }
        None => (s, vec![]),
    };
    let num = |i: usize| -> Result<usize, String> {
        args.get(i)
            .and_then(|a| a.parse().ok())
            .ok_or_else(|| format!("过滤器「{s}」的第 {} 个参数应为数字", i + 1))
    };
    let string = |i: usize| -> Result<String, String> {
        args.get(i)
            .and_then(|a| unquote(a))
            .ok_or_else(|| format!("过滤器「{s}」的第 {} 个参数应为带引号的文字", i + 1))
    };
    Ok(match name {
        "lower" => Filter::Lower,
        "upper" => Filter::Upper,
        "title" => Filter::Title,
        "slug" => Filter::Slug,
        "trim" => Filter::Trim,
        "pad" => Filter::Pad(num(0)?),
        "slice" => Filter::Slice(num(0)?, if args.len() > 1 { Some(num(1)?) } else { None }),
        "replace" => {
            let pat = string(0)?;
            Filter::Replace(
                Regex::new(&pat).map_err(|e| format!("replace 的正则「{pat}」无效: {e}"))?,
                string(1)?,
            )
        }
        "default" => Filter::Default(string(0)?),
        other => {
            return Err(format!(
                "未知过滤器「{other}」，可用：lower upper title slug trim pad(n) slice(a,b) replace(\"正则\",\"替换\") default(\"值\")"
            ));
        }
    })
}

/// 依次尝试备选，返回 (值, 是否原样输出)。都取不到时返回 None。
fn eval_alts(alts: &[Alt], ctx: &TemplateCtx) -> Result<Option<(String, bool)>, String> {
    for a in alts {
        let hit = match a {
            Alt::Lit(s) => Some((s.clone(), false)),
            Alt::Var { var, fmt } => eval_var(var, fmt.as_deref(), ctx).map(|v| (v, is_raw(var))),
        };
        if let Some((v, raw)) = hit
            && !v.is_empty()
        {
            return Ok(Some((v, raw)));
        }
    }
    Ok(None)
}

fn is_raw(v: &Var) -> bool {
    matches!(v, Var::Env(_) | Var::Dir(_)) || matches!(v, Var::Field(f) if f.key == "path")
}

fn eval_var(var: &Var, fmt: Option<&str>, ctx: &TemplateCtx) -> Option<String> {
    let date = |d: DateTime<Local>| d.format(fmt.unwrap_or(DEFAULT_DATE_FMT)).to_string();
    match var {
        Var::Field(f) => (f.get)(ctx.file).map(|v| match v {
            Value::Date(d) => date(d),
            other => other.display(),
        }),
        Var::Cur(c) => {
            let (name, stem, ext) = split_name(ctx.cur);
            Some(match c {
                CurVar::Name => name,
                CurVar::Stem => stem,
                CurVar::Ext => ext,
                CurVar::Parent => ctx
                    .cur
                    .parent()
                    .and_then(|p| p.file_name())
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            })
        }
        Var::Now => Some(date(ctx.now)),
        Var::Match(k) => ctx.captures.get(k).cloned(),
        Var::Env(k) => std::env::var(k).ok(),
        Var::Dir(d) => {
            let p = match *d {
                "home" => dirs::home_dir(),
                "desktop" => dirs::desktop_dir(),
                "documents" => dirs::document_dir(),
                "downloads" => dirs::download_dir(),
                "pictures" => dirs::picture_dir(),
                "music" => dirs::audio_dir(),
                "videos" => dirs::video_dir(),
                _ => None,
            };
            p.map(|p| p.to_string_lossy().into_owned())
        }
        Var::System(s) => match *s {
            "user" => std::env::var("USER")
                .or_else(|_| std::env::var("USERNAME"))
                .ok(),
            "host" => Some(gethostname::gethostname().to_string_lossy().into_owned()),
            "os" => Some(
                match std::env::consts::OS {
                    "macos" => "macOS",
                    "windows" => "Windows",
                    "linux" => "Linux",
                    other => other,
                }
                .to_string(),
            ),
            _ => None,
        },
    }
}

fn apply_filter(f: &Filter, s: &str) -> String {
    match f {
        Filter::Lower => s.to_lowercase(),
        Filter::Upper => s.to_uppercase(),
        Filter::Trim => s.trim().to_string(),
        Filter::Slug => slug(s),
        Filter::Title => s
            .split(' ')
            .map(|w| {
                let mut c = w.chars();
                match c.next() {
                    Some(first) => first
                        .to_uppercase()
                        .chain(c.flat_map(char::to_lowercase))
                        .collect(),
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
        Filter::Pad(n) => {
            let len = s.chars().count();
            if len >= *n {
                s.to_string()
            } else {
                format!("{}{s}", "0".repeat(n - len))
            }
        }
        Filter::Slice(a, b) => {
            let chars: Vec<char> = s.chars().collect();
            let end = b.unwrap_or(chars.len()).min(chars.len());
            let start = (*a).min(end);
            chars[start..end].iter().collect()
        }
        Filter::Replace(re, rep) => re.replace_all(s, rep.as_str()).into_owned(),
        Filter::Default(_) => s.to_string(),
    }
}

/// 小写，非字母数字（含中文等 Unicode 文字保留）替换为 `-`，合并连续的 `-`。
pub fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.extend(c.to_lowercase());
        } else {
            pending_dash = true;
        }
    }
    out
}

/// 变量值里去掉在 Windows / macOS 上不合法或有特殊含义的字符。
/// `/` 和 `\` 保留，这样 `{created:%Y/%m}` 可以生成多级目录。
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fileinfo::FileInfo;
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn file() -> FileCtx {
        let t = Local.with_ymd_and_hms(2024, 5, 17, 9, 30, 0).unwrap();
        FileInfo {
            path: PathBuf::from("/data/in/IMG_0001.JPG"),
            name: "IMG_0001.JPG".into(),
            stem: "IMG_0001".into(),
            ext: "JPG".into(),
            size: 1234,
            created: Some(t),
            modified: Some(t),
            accessed: None,
            hidden: false,
            readonly: false,
        }
        .into()
    }

    fn render(tpl: &str) -> Result<String, String> {
        let f = file();
        let mut caps = BTreeMap::new();
        caps.insert("1".to_string(), "0001".to_string());
        caps.insert("year".to_string(), "2024".to_string());
        let ctx = TemplateCtx {
            cur: &f.path,
            file: &f,
            captures: &caps,
            now: Local.with_ymd_and_hms(2026, 1, 2, 3, 4, 5).unwrap(),
        };
        Template::parse(tpl)?.render(&ctx)
    }

    #[test]
    fn variables_and_filters() {
        assert_eq!(render("{created:%Y}/{created:%m}/").unwrap(), "2024/05/");
        assert_eq!(render("{stem}.{ext|lower}").unwrap(), "IMG_0001.jpg");
        assert_eq!(render("{name|slug}").unwrap(), "img-0001-jpg");
        assert_eq!(render("{parent}-{size}").unwrap(), "in-1234");
        assert_eq!(render("{modified}").unwrap(), "2024-05-17");
        assert_eq!(render("{now:%H:%M}").unwrap(), "03_04");
        assert_eq!(
            render("photo_{regex.1}_{match.year}").unwrap(),
            "photo_0001_2024"
        );
        assert_eq!(render("{category}").unwrap(), "图片");
        assert_eq!(
            render("{modified.year}-{modified.month|pad(2)}").unwrap(),
            "2024-05"
        );
        assert_eq!(render("{stem|slice(0,3)|lower}").unwrap(), "img");
        assert_eq!(
            render(r#"{stem|replace("_(\d+)$","-$1")}"#).unwrap(),
            "IMG-0001"
        );
        assert_eq!(render("{{literal}}").unwrap(), "{literal}");
    }

    #[test]
    fn fallbacks_and_defaults() {
        // accessed 为空 → 用 modified，并沿用最后一个备选的格式
        assert_eq!(render("{accessed ?? modified:%Y}").unwrap(), "2024");
        assert_eq!(render(r#"{exif.model ?? "未知相机"}"#).unwrap(), "未知相机");
        assert_eq!(render(r#"{exif.model|default("无")}"#).unwrap(), "无");
        assert!(render("{accessed}").is_err());
        assert!(render("{match.missing}").is_err());
    }

    #[test]
    fn system_variables() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(render("{dir.home}").unwrap(), home.to_string_lossy());
        // 路径类变量原样输出（Windows 的 C:\ 不能被替换）
        assert!(
            !render("{dir.home}").unwrap().contains('_') || home.to_string_lossy().contains('_')
        );
        assert!(!render("{system.os}").unwrap().is_empty());
        assert!(!render("{system.host}").unwrap().is_empty());
        unsafe { std::env::set_var("FOLDO_TEST_VAR", "a:b") };
        assert_eq!(render("{env.FOLDO_TEST_VAR}").unwrap(), "a:b");
    }

    #[test]
    fn errors() {
        assert!(Template::parse("{unknown}").is_err());
        assert!(Template::parse("{name:%Y}").is_err());
        assert!(Template::parse("{created:%Q}").is_err());
        assert!(Template::parse("{name|shout}").is_err());
        assert!(Template::parse("{name|pad(x)}").is_err());
        assert!(Template::parse(r#"{name|replace("(","")}"#).is_err());
        assert!(Template::parse("{dir.nowhere}").is_err());
        assert!(Template::parse("{name").is_err());
        assert!(Template::parse("a}b").is_err());
    }

    #[test]
    fn slug_keeps_unicode() {
        assert_eq!(slug("年度 报告 (Final)!"), "年度-报告-final");
    }
}
