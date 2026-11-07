//! 路径模板：`~/Pictures/{created:%Y}/{created:%m}/`、`{stem|slug}.{ext|lower}`。
//!
//! 语法：`{变量[:格式][|过滤器]...}`，用 `{{` / `}}` 输出字面量花括号。
//!
//! 变量：`name` `stem` `ext` `parent` `size` `created` `modified` `accessed` `now` `regex.N` `regex.组名`
//! 时间变量的格式使用 strftime（默认 `%Y-%m-%d`）。
//! 过滤器：`lower` `upper` `slug` `trim`

use crate::fileinfo::{FileInfo, split_name};
use chrono::format::{Item, StrftimeItems};
use chrono::{DateTime, Local};
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
    Var {
        var: Var,
        fmt: Option<String>,
        filters: Vec<Filter>,
    },
}

#[derive(Debug, Clone)]
enum Var {
    Name,
    Stem,
    Ext,
    Parent,
    Size,
    Created,
    Modified,
    Accessed,
    Now,
    Regex(String),
}

#[derive(Debug, Clone, Copy)]
enum Filter {
    Lower,
    Upper,
    Slug,
    Trim,
}

/// 渲染模板需要的信息。`cur` 是文件当前（可能已被前面的动作改过名）的路径。
pub struct TemplateCtx<'a> {
    pub cur: &'a Path,
    pub file: &'a FileInfo,
    pub captures: &'a BTreeMap<String, String>,
    pub now: DateTime<Local>,
}

const DEFAULT_DATE_FMT: &str = "%Y-%m-%d";

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
                    let mut expr = String::new();
                    let mut closed = false;
                    for c in chars.by_ref() {
                        if c == '}' {
                            closed = true;
                            break;
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

    pub fn render(&self, ctx: &TemplateCtx) -> Result<String, String> {
        let mut out = String::new();
        for p in &self.parts {
            match p {
                Part::Lit(s) => out.push_str(s),
                Part::Var { var, fmt, filters } => {
                    let mut v = eval(var, fmt.as_deref(), ctx)?;
                    for f in filters {
                        v = apply_filter(*f, &v);
                    }
                    out.push_str(&sanitize(&v));
                }
            }
        }
        Ok(out)
    }
}

fn parse_expr(expr: &str) -> Result<Part, String> {
    let mut segs = expr.split('|');
    let head = segs.next().unwrap_or("").trim();
    let (name, fmt) = match head.split_once(':') {
        Some((n, f)) => (n.trim(), Some(f.to_string())),
        None => (head, None),
    };
    let var = match name {
        "name" => Var::Name,
        "stem" => Var::Stem,
        "ext" => Var::Ext,
        "parent" => Var::Parent,
        "size" => Var::Size,
        "created" => Var::Created,
        "modified" => Var::Modified,
        "accessed" => Var::Accessed,
        "now" => Var::Now,
        n if n.starts_with("regex.") && n.len() > 6 => Var::Regex(n[6..].to_string()),
        "" => return Err("空的变量 {}".into()),
        other => return Err(format!("未知变量「{other}」")),
    };
    let is_date = matches!(var, Var::Created | Var::Modified | Var::Accessed | Var::Now);
    if let Some(f) = &fmt {
        if !is_date {
            return Err(format!("变量「{name}」不支持格式参数"));
        }
        if StrftimeItems::new(f).any(|i| matches!(i, Item::Error)) {
            return Err(format!("时间格式「{f}」无效"));
        }
    }
    let filters = segs
        .map(|s| match s.trim() {
            "lower" => Ok(Filter::Lower),
            "upper" => Ok(Filter::Upper),
            "slug" => Ok(Filter::Slug),
            "trim" => Ok(Filter::Trim),
            other => Err(format!(
                "未知过滤器「{other}」，可用：lower upper slug trim"
            )),
        })
        .collect::<Result<_, _>>()?;
    Ok(Part::Var { var, fmt, filters })
}

fn eval(var: &Var, fmt: Option<&str>, ctx: &TemplateCtx) -> Result<String, String> {
    let date = |t: Option<DateTime<Local>>, what: &str| {
        t.map(|t| t.format(fmt.unwrap_or(DEFAULT_DATE_FMT)).to_string())
            .ok_or_else(|| format!("无法获取文件的{what}"))
    };
    let (name, stem, ext) = split_name(ctx.cur);
    Ok(match var {
        Var::Name => name,
        Var::Stem => stem,
        Var::Ext => ext,
        Var::Parent => ctx
            .cur
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        Var::Size => ctx.file.size.to_string(),
        Var::Created => date(ctx.file.created, "创建时间")?,
        Var::Modified => date(ctx.file.modified, "修改时间")?,
        Var::Accessed => date(ctx.file.accessed, "访问时间")?,
        Var::Now => date(Some(ctx.now), "当前时间")?,
        Var::Regex(k) => ctx
            .captures
            .get(k)
            .cloned()
            .ok_or_else(|| format!("没有正则捕获组「{k}」"))?,
    })
}

fn apply_filter(f: Filter, s: &str) -> String {
    match f {
        Filter::Lower => s.to_lowercase(),
        Filter::Upper => s.to_uppercase(),
        Filter::Trim => s.trim().to_string(),
        Filter::Slug => slug(s),
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

/// 变量值里去掉在 Windows / macOS 上不合法或有特殊含义的字符，保证规则在三个平台上行为一致。
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
    use chrono::TimeZone;
    use std::path::PathBuf;

    fn file() -> FileInfo {
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
        }
    }

    fn render(tpl: &str) -> Result<String, String> {
        let f = file();
        let mut caps = BTreeMap::new();
        caps.insert("1".to_string(), "0001".to_string());
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
        assert_eq!(render("photo_{regex.1}").unwrap(), "photo_0001");
        assert_eq!(render("{{literal}}").unwrap(), "{literal}");
    }

    #[test]
    fn errors() {
        assert!(Template::parse("{unknown}").is_err());
        assert!(Template::parse("{name:%Y}").is_err());
        assert!(Template::parse("{created:%Q}").is_err());
        assert!(Template::parse("{name|shout}").is_err());
        assert!(Template::parse("{name").is_err());
        assert!(Template::parse("a}b").is_err());
        assert!(render("{accessed}").is_err());
        assert!(render("{regex.missing}").is_err());
    }

    #[test]
    fn slug_keeps_unicode() {
        assert_eq!(slug("年度 报告 (Final)!"), "年度-报告-final");
    }
}
