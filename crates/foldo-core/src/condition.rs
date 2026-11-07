//! 条件引擎：任意嵌套的“与 / 或 / 都不 / 非”，以及“字段 + 运算符 + 值”的比较。
//!
//! 写法（YAML）：
//! ```yaml
//! all:
//!   - category: 图片                      # 简写：等于
//!   - ext: [jpg, png]                     # 简写：在列表中
//!   - image.width: { ">=": 3000 }
//!   - name: { matches: '^IMG_(?P<y>\d{4})', case_sensitive: true }
//!   - modified: { older_than: 30d }
//!   - any: [ { size: { gt: 10MB } }, { exif.gps: true } ]
//!   - not: { name: { contains: 草稿 } }
//! ```
//! 一个字段可以同时写多个运算符（都要满足），如 `size: { gt: 1MB, lt: 1GB }`。
//! 文本比较默认不区分大小写；比较的值里可以写变量，如 `parent: { eq: "{modified.year}" }`。
//! 旧写法（`extension`、`name: "*.pdf"`、`name: { regex: … }`、`size: { gt: … }`、
//! `modified: { older_than: … }`）都是这套写法的特例，可以直接使用。

use crate::categories;
use crate::config::ConditionDef;
use crate::fields::{self, FieldDef, FieldType, ORIENTATIONS, Value};
use crate::meta::FileCtx;
use crate::template::{Template, TemplateCtx};
use crate::units::{ByteSize, Span};
use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, NaiveDateTime, TimeZone,
};
use globset::{GlobBuilder, GlobMatcher};
use regex::{Regex, RegexBuilder};
use serde_json::Value as Json;
use std::collections::BTreeMap;
use std::time::Duration;

/// 匹配过程中的上下文；正则的捕获组存进 `captures`，供路径里的 `{match.1}` 使用。
pub struct MatchCtx {
    pub now: DateTime<Local>,
    pub captures: BTreeMap<String, String>,
}

impl MatchCtx {
    pub fn new(now: DateTime<Local>) -> Self {
        Self {
            now,
            captures: BTreeMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum Condition {
    All(Vec<Condition>),
    Any(Vec<Condition>),
    /// 都不满足
    None(Vec<Condition>),
    Not(Box<Condition>),
    Test(Test),
}

#[derive(Debug)]
pub struct Test {
    pub field: &'static FieldDef,
    checks: Vec<Check>,
    case_sensitive: bool,
}

#[derive(Debug)]
enum Operand {
    Lit(String),
    Tpl(Template),
}

#[derive(Debug, Clone, Copy)]
enum TextOp {
    Eq,
    Ne,
    Contains,
    NotContains,
    StartsWith,
    EndsWith,
}

#[derive(Debug, Clone, Copy)]
enum Cmp {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
}

#[derive(Debug, Clone, Copy)]
enum Period {
    Today,
    Yesterday,
    ThisWeek,
    LastWeek,
    ThisMonth,
    LastMonth,
    ThisYear,
    LastYear,
}

#[derive(Debug)]
enum Check {
    Exists(bool),
    Empty(bool),
    Text(TextOp, Operand),
    In(Vec<Operand>, bool),
    Glob(GlobMatcher),
    Regex(Regex),
    Num(Cmp, f64),
    NumBetween(f64, f64),
    Date(Cmp, DateTime<Local>),
    DateBetween(DateTime<Local>, DateTime<Local>),
    OlderThan(Duration),
    NewerThan(Duration),
    Within(Period),
    Bool(bool),
}

// ---------- 编译 ----------

const GROUPS: &[&str] = &["all", "any", "none", "not"];

impl Condition {
    pub fn compile(def: &ConditionDef) -> Result<Self, String> {
        match def.key.as_str() {
            k @ ("all" | "any" | "none") => {
                let items = def
                    .value
                    .as_array()
                    .ok_or_else(|| format!("{k} 后面应是条件列表"))?;
                let list = items
                    .iter()
                    .map(|v| {
                        serde_json::from_value::<ConditionDef>(v.clone())
                            .map_err(|e| e.to_string())
                            .and_then(|d| Condition::compile(&d))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if list.is_empty() && k != "all" {
                    return Err(format!("{k} 至少需要一个条件"));
                }
                Ok(match k {
                    "all" => Condition::All(list),
                    "any" => Condition::Any(list),
                    _ => Condition::None(list),
                })
            }
            "not" => {
                let inner: ConditionDef = serde_json::from_value(def.value.clone())
                    .map_err(|e| format!("not 后面应是一个条件: {e}"))?;
                Ok(Condition::Not(Box::new(Condition::compile(&inner)?)))
            }
            key => {
                let field = fields::field(key).ok_or_else(|| {
                    format!("未知字段「{key}」（条件组请用 {}）", GROUPS.join(" / "))
                })?;
                Ok(Condition::Test(compile_test(field, &def.value)?))
            }
        }
    }

    /// 是否用到某个字段（用于说明和性能测试）。
    pub fn uses(&self, key: &str) -> bool {
        match self {
            Condition::All(l) | Condition::Any(l) | Condition::None(l) => {
                l.iter().any(|c| c.uses(key))
            }
            Condition::Not(c) => c.uses(key),
            Condition::Test(t) => t.field.key == key,
        }
    }
}

fn op_names(ty: FieldType) -> &'static str {
    match ty {
        FieldType::Text | FieldType::Enum => {
            "eq ne contains not_contains starts_with ends_with in not_in glob matches empty exists"
        }
        FieldType::Number | FieldType::Size => {
            "eq ne gt gte lt lte between exists（也可写 = != > >= < <=）"
        }
        FieldType::Date => "before after on between older_than newer_than within exists",
        FieldType::Bool => "eq exists",
    }
}

fn canonical(op: &str) -> &str {
    match op {
        "=" | "==" | "is" | "equals" => "eq",
        "!=" | "≠" | "<>" | "is_not" => "ne",
        ">" => "gt",
        ">=" | "≥" => "gte",
        "<" => "lt",
        "<=" | "≤" => "lte",
        "regex" => "matches",
        "is_empty" => "empty",
        o => o,
    }
}

fn compile_test(field: &'static FieldDef, value: &Json) -> Result<Test, String> {
    let mut case_sensitive = false;
    let mut checks = Vec::new();
    match value {
        Json::Object(map) => {
            if let Some(cs) = map.get("case_sensitive") {
                case_sensitive = cs.as_bool().ok_or("case_sensitive 应为 true 或 false")?;
            }
            for (op, v) in map {
                if op == "case_sensitive" {
                    continue;
                }
                checks.push(compile_op(field, canonical(op), v, case_sensitive)?);
            }
            if checks.is_empty() {
                return Err(format!(
                    "「{}」没有写比较方式，可用：{}",
                    field.label,
                    op_names(field.ty)
                ));
            }
        }
        Json::Array(_) => checks.push(compile_op(field, "in", value, false)?),
        Json::Null => return Err(format!("「{}」缺少要比较的值", field.label)),
        scalar => {
            // 简写：文本里带 * ? 时按通配符，日期按“当天”，其余按“等于”
            let op = match field.ty {
                FieldType::Text if scalar.as_str().is_some_and(|s| s.contains(['*', '?'])) => {
                    "glob"
                }
                FieldType::Date => "on",
                _ => "eq",
            };
            checks.push(compile_op(field, op, scalar, false)?);
        }
    }
    Ok(Test {
        field,
        checks,
        case_sensitive,
    })
}

fn as_text(v: &Json) -> Option<String> {
    match v {
        Json::String(s) => Some(s.clone()),
        Json::Number(n) => Some(n.to_string()),
        Json::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// 把枚举值规范化（分类认别名，方向认英文）。
fn normalize_enum(field: &FieldDef, s: &str) -> Result<String, String> {
    match field.key {
        "category" => categories::find(s)
            .map(|c| c.name.to_string())
            .ok_or_else(|| format!("未知类型「{s}」，可用：{}", categories::names())),
        "image.orientation" => {
            let v = match s.trim().to_lowercase().as_str() {
                "横" | "横向" | "横版" | "landscape" | "horizontal" => "横",
                "竖" | "纵向" | "竖版" | "portrait" | "vertical" => "竖",
                "方" | "正方形" | "square" => "方",
                _ => {
                    return Err(format!(
                        "未知方向「{s}」，可用：{}",
                        ORIENTATIONS.join(" / ")
                    ));
                }
            };
            Ok(v.into())
        }
        _ => Ok(s.to_string()),
    }
}

fn operand(field: &FieldDef, v: &Json, cs: bool) -> Result<Operand, String> {
    let s = as_text(v).ok_or_else(|| format!("「{}」的比较值应为文字", field.label))?;
    let s = if field.ty == FieldType::Enum {
        normalize_enum(field, &s)?
    } else {
        s
    };
    // 扩展名允许写成 .jpg
    let s = if field.key == "ext" {
        s.trim_start_matches('.').to_string()
    } else {
        s
    };
    if s.contains('{') {
        let t = Template::parse(&s)?;
        if t.has_vars() {
            return Ok(Operand::Tpl(t));
        }
    }
    Ok(Operand::Lit(if cs { s } else { s.to_lowercase() }))
}

fn parse_number(field: &FieldDef, v: &Json) -> Result<f64, String> {
    let bad = || format!("「{}」的比较值应为数字", field.label);
    if field.ty == FieldType::Size {
        return match v {
            Json::Number(n) => n.as_f64().ok_or_else(bad),
            Json::String(s) => s.parse::<ByteSize>().map(|b| b.0 as f64),
            _ => Err(bad()),
        };
    }
    match v {
        Json::Number(n) => n.as_f64().ok_or_else(bad),
        Json::String(s) => s.trim().parse::<f64>().or_else(|_| {
            // 时长字段可以写 90s、5m、1h
            if field.unit == Some("秒") {
                s.parse::<Span>().map(|d| d.0.as_secs_f64())
            } else {
                Err(bad())
            }
        }),
        _ => Err(bad()),
    }
}

/// 解析日期，返回 (时间, 是否只写了日期)。
fn parse_date(v: &Json) -> Result<(DateTime<Local>, bool), String> {
    let s = as_text(v).ok_or("日期应写成 2026-01-31 或 2026-01-31 18:00")?;
    let s = s.trim();
    for f in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y/%m/%d %H:%M",
    ] {
        if let Ok(n) = NaiveDateTime::parse_from_str(s, f) {
            return local(n).map(|d| (d, false));
        }
    }
    for f in ["%Y-%m-%d", "%Y/%m/%d", "%Y%m%d"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, f) {
            return local(d.and_hms_opt(0, 0, 0).expect("合法时间")).map(|d| (d, true));
        }
    }
    Err(format!(
        "无法识别日期「{s}」，请写成 2026-01-31 或 2026-01-31 18:00"
    ))
}

fn local(n: NaiveDateTime) -> Result<DateTime<Local>, String> {
    Local
        .from_local_datetime(&n)
        .earliest()
        .ok_or_else(|| format!("本地时间 {n} 不存在（夏令时切换）"))
}

fn parse_span(field: &FieldDef, v: &Json) -> Result<Duration, String> {
    let s = as_text(v).ok_or_else(|| format!("「{}」的时长应写成 30d、12h 这样", field.label))?;
    s.parse::<Span>().map(|s| s.0)
}

fn parse_period(v: &Json) -> Result<Period, String> {
    let s = as_text(v).unwrap_or_default();
    Ok(match s.trim() {
        "today" | "今天" => Period::Today,
        "yesterday" | "昨天" => Period::Yesterday,
        "this_week" | "本周" => Period::ThisWeek,
        "last_week" | "上周" => Period::LastWeek,
        "this_month" | "本月" => Period::ThisMonth,
        "last_month" | "上月" | "上个月" => Period::LastMonth,
        "this_year" | "今年" => Period::ThisYear,
        "last_year" | "去年" => Period::LastYear,
        other => {
            return Err(format!(
                "未知时间段「{other}」，可用：today yesterday this_week last_week this_month last_month this_year last_year（或 今天 昨天 本周 上周 本月 上月 今年 去年）"
            ));
        }
    })
}

fn pair<'a>(field: &FieldDef, v: &'a Json) -> Result<(&'a Json, &'a Json), String> {
    match v.as_array().map(Vec::as_slice) {
        Some([a, b]) => Ok((a, b)),
        _ => Err(format!(
            "「{}」的 between 应写成 [最小值, 最大值]",
            field.label
        )),
    }
}

fn compile_op(field: &'static FieldDef, op: &str, v: &Json, cs: bool) -> Result<Check, String> {
    let unsupported = || {
        format!(
            "「{}」不支持「{op}」，可用：{}",
            field.label,
            op_names(field.ty)
        )
    };
    if op == "exists" {
        return Ok(Check::Exists(
            v.as_bool().ok_or("exists 应为 true 或 false")?,
        ));
    }
    match field.ty {
        FieldType::Text | FieldType::Enum => Ok(match op {
            "eq" => Check::Text(TextOp::Eq, operand(field, v, cs)?),
            "ne" => Check::Text(TextOp::Ne, operand(field, v, cs)?),
            "contains" => Check::Text(TextOp::Contains, operand(field, v, cs)?),
            "not_contains" => Check::Text(TextOp::NotContains, operand(field, v, cs)?),
            "starts_with" => Check::Text(TextOp::StartsWith, operand(field, v, cs)?),
            "ends_with" => Check::Text(TextOp::EndsWith, operand(field, v, cs)?),
            "in" | "not_in" => {
                let items: Vec<&Json> = match v {
                    Json::Array(a) => a.iter().collect(),
                    other => vec![other],
                };
                if items.is_empty() {
                    return Err(format!("「{}」的列表不能为空", field.label));
                }
                Check::In(
                    items
                        .into_iter()
                        .map(|x| operand(field, x, cs))
                        .collect::<Result<_, _>>()?,
                    op == "not_in",
                )
            }
            "glob" => {
                let g = as_text(v).ok_or("通配符应为文字")?;
                Check::Glob(
                    GlobBuilder::new(&g)
                        .case_insensitive(!cs)
                        .build()
                        .map_err(|e| format!("通配符「{g}」无效: {e}"))?
                        .compile_matcher(),
                )
            }
            "matches" => {
                let r = as_text(v).ok_or("正则应为文字")?;
                Check::Regex(
                    RegexBuilder::new(&r)
                        .case_insensitive(!cs)
                        .build()
                        .map_err(|e| format!("正则「{r}」无效: {e}"))?,
                )
            }
            "empty" => Check::Empty(v.as_bool().ok_or("empty 应为 true 或 false")?),
            _ => return Err(unsupported()),
        }),
        FieldType::Number | FieldType::Size => {
            let cmp = match op {
                "eq" => Cmp::Eq,
                "ne" => Cmp::Ne,
                "gt" => Cmp::Gt,
                "gte" => Cmp::Gte,
                "lt" => Cmp::Lt,
                "lte" => Cmp::Lte,
                "between" => {
                    let (a, b) = pair(field, v)?;
                    let (a, b) = (parse_number(field, a)?, parse_number(field, b)?);
                    return Ok(Check::NumBetween(a.min(b), a.max(b)));
                }
                _ => return Err(unsupported()),
            };
            Ok(Check::Num(cmp, parse_number(field, v)?))
        }
        FieldType::Date => Ok(match op {
            "before" | "lt" => Check::Date(Cmp::Lt, parse_date(v)?.0),
            "after" | "gt" => {
                // 只写日期时，“晚于 1 月 31 日”指 2 月 1 日 0 点以后
                let (d, date_only) = parse_date(v)?;
                if date_only {
                    Check::Date(Cmp::Gte, d + ChronoDuration::days(1))
                } else {
                    Check::Date(Cmp::Gt, d)
                }
            }
            "on" | "eq" => {
                let (d, _) = parse_date(v)?;
                let start = local(d.date_naive().and_hms_opt(0, 0, 0).expect("合法时间"))?;
                Check::DateBetween(start, start + ChronoDuration::days(1))
            }
            "between" => {
                let (a, b) = pair(field, v)?;
                let (a, _) = parse_date(a)?;
                let (b, b_date_only) = parse_date(b)?;
                let end = if b_date_only {
                    b + ChronoDuration::days(1)
                } else {
                    b
                };
                Check::DateBetween(a.min(end), a.max(end))
            }
            "older_than" => Check::OlderThan(parse_span(field, v)?),
            "newer_than" => Check::NewerThan(parse_span(field, v)?),
            "within" => Check::Within(parse_period(v)?),
            _ => return Err(unsupported()),
        }),
        FieldType::Bool => match op {
            "eq" => Ok(Check::Bool(match v {
                Json::Bool(b) => *b,
                Json::String(s) if matches!(s.as_str(), "是" | "true" | "yes") => true,
                Json::String(s) if matches!(s.as_str(), "否" | "false" | "no") => false,
                _ => return Err(format!("「{}」应为 true 或 false", field.label)),
            })),
            _ => Err(unsupported()),
        },
    }
}

// ---------- 解释（“用文件测试”） ----------

/// 条件树中每个节点对某个文件是否成立。结构与配置里的条件树一一对应。
#[derive(Debug, Clone, serde::Serialize)]
pub struct Explain {
    pub ok: bool,
    /// 条件写错时的说明（此时 ok 为 false）。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// 组（all / any / none / not）的子节点结果。
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Explain>,
}

/// 按配置逐个节点求值，供界面在每个条件旁边显示成立与否。
pub fn explain(def: &ConditionDef, f: &FileCtx, ctx: &mut MatchCtx) -> Explain {
    let fail = |e: String| Explain {
        ok: false,
        error: Some(e),
        children: vec![],
    };
    match def.key.as_str() {
        k @ ("all" | "any" | "none") => {
            let Some(items) = def.value.as_array() else {
                return fail(format!("{k} 后面应是条件列表"));
            };
            let children: Vec<Explain> = items
                .iter()
                .map(
                    |v| match serde_json::from_value::<ConditionDef>(v.clone()) {
                        Ok(d) => explain(&d, f, ctx),
                        Err(e) => fail(e.to_string()),
                    },
                )
                .collect();
            let ok = match k {
                "all" => children.iter().all(|c| c.ok),
                "any" => children.iter().any(|c| c.ok),
                _ => !children.iter().any(|c| c.ok),
            };
            Explain {
                ok,
                error: None,
                children,
            }
        }
        "not" => match serde_json::from_value::<ConditionDef>(def.value.clone()) {
            Ok(inner) => {
                let child = explain(&inner, f, ctx);
                Explain {
                    ok: child.error.is_none() && !child.ok,
                    error: None,
                    children: vec![child],
                }
            }
            Err(e) => fail(e.to_string()),
        },
        _ => match Condition::compile(def) {
            Ok(c) => Explain {
                ok: c.matches(f, ctx),
                error: None,
                children: vec![],
            },
            Err(e) => fail(e),
        },
    }
}

// ---------- 匹配 ----------

impl Condition {
    pub fn matches(&self, f: &FileCtx, ctx: &mut MatchCtx) -> bool {
        match self {
            Condition::All(list) => list.iter().all(|c| c.matches(f, ctx)),
            Condition::Any(list) => list.iter().any(|c| c.matches(f, ctx)),
            Condition::None(list) => !list.iter().any(|c| c.matches(f, ctx)),
            Condition::Not(c) => !c.matches(f, ctx),
            Condition::Test(t) => t.matches(f, ctx),
        }
    }
}

impl Test {
    fn matches(&self, f: &FileCtx, ctx: &mut MatchCtx) -> bool {
        let value = (self.field.get)(f);
        self.checks
            .iter()
            .all(|c| self.check(c, value.as_ref(), f, ctx))
    }

    fn resolve(&self, op: &Operand, f: &FileCtx, ctx: &MatchCtx) -> Option<String> {
        match op {
            Operand::Lit(s) => Some(s.clone()),
            Operand::Tpl(t) => {
                let tctx = TemplateCtx {
                    cur: &f.path,
                    file: f,
                    captures: &ctx.captures,
                    now: ctx.now,
                };
                let s = t.render_raw(&tctx).ok()?;
                Some(if self.case_sensitive {
                    s
                } else {
                    s.to_lowercase()
                })
            }
        }
    }

    fn check(&self, c: &Check, value: Option<&Value>, f: &FileCtx, ctx: &mut MatchCtx) -> bool {
        if let Check::Exists(b) = c {
            return value.is_some() == *b;
        }
        let text = || {
            value.map(|v| {
                let s = match v {
                    Value::Text(s) => s.clone(),
                    other => other.display(),
                };
                if self.case_sensitive {
                    s
                } else {
                    s.to_lowercase()
                }
            })
        };
        match c {
            Check::Exists(_) => unreachable!(),
            Check::Empty(b) => text().unwrap_or_default().is_empty() == *b,
            Check::Text(op, rhs) => {
                let (Some(lhs), Some(rhs)) = (text(), self.resolve(rhs, f, ctx)) else {
                    return false;
                };
                match op {
                    TextOp::Eq => lhs == rhs,
                    TextOp::Ne => lhs != rhs,
                    TextOp::Contains => lhs.contains(&rhs),
                    TextOp::NotContains => !lhs.contains(&rhs),
                    TextOp::StartsWith => lhs.starts_with(&rhs),
                    TextOp::EndsWith => lhs.ends_with(&rhs),
                }
            }
            Check::In(items, negate) => {
                let Some(lhs) = text() else { return false };
                let hit = items
                    .iter()
                    .any(|o| self.resolve(o, f, ctx).is_some_and(|r| r == lhs));
                hit != *negate
            }
            Check::Glob(g) => match value {
                Some(Value::Text(s)) => g.is_match(s),
                Some(v) => g.is_match(v.display()),
                None => false,
            },
            Check::Regex(re) => {
                let s = match value {
                    Some(Value::Text(s)) => s.clone(),
                    Some(v) => v.display(),
                    None => return false,
                };
                let Some(caps) = re.captures(&s) else {
                    return false;
                };
                for (i, m) in caps.iter().enumerate() {
                    if let Some(m) = m {
                        ctx.captures.insert(i.to_string(), m.as_str().to_string());
                    }
                }
                for name in re.capture_names().flatten() {
                    if let Some(m) = caps.name(name) {
                        ctx.captures
                            .insert(name.to_string(), m.as_str().to_string());
                    }
                }
                true
            }
            Check::Num(cmp, rhs) => {
                let lhs = match value {
                    Some(Value::Number(n)) => *n,
                    Some(Value::Size(b)) => *b as f64,
                    _ => return false,
                };
                compare(lhs, *rhs, *cmp)
            }
            Check::NumBetween(a, b) => match value {
                Some(Value::Number(n)) => *a <= *n && *n <= *b,
                Some(Value::Size(s)) => *a <= *s as f64 && *s as f64 <= *b,
                _ => false,
            },
            Check::Date(cmp, rhs) => match value {
                Some(Value::Date(d)) => compare(*d, *rhs, *cmp),
                _ => false,
            },
            Check::DateBetween(a, b) => match value {
                Some(Value::Date(d)) => a <= d && d < b,
                _ => false,
            },
            Check::OlderThan(dur) => match value {
                Some(Value::Date(d)) => (ctx.now - *d).to_std().unwrap_or_default() >= *dur,
                _ => false,
            },
            Check::NewerThan(dur) => match value {
                Some(Value::Date(d)) => (ctx.now - *d).to_std().unwrap_or_default() < *dur,
                _ => false,
            },
            Check::Within(p) => match value {
                Some(Value::Date(d)) => {
                    let (a, b) = period_range(*p, ctx.now);
                    a <= *d && *d < b
                }
                _ => false,
            },
            Check::Bool(b) => matches!(value, Some(Value::Bool(v)) if v == b),
        }
    }
}

fn compare<T: PartialOrd>(a: T, b: T, cmp: Cmp) -> bool {
    match cmp {
        Cmp::Eq => a == b,
        Cmp::Ne => a != b,
        Cmp::Gt => a > b,
        Cmp::Gte => a >= b,
        Cmp::Lt => a < b,
        Cmp::Lte => a <= b,
    }
}

fn midnight(d: NaiveDate) -> DateTime<Local> {
    local(d.and_hms_opt(0, 0, 0).expect("合法时间")).unwrap_or_else(|_| Local::now())
}

/// 日历时间段 [开始, 结束)。一周从周一开始。
fn period_range(p: Period, now: DateTime<Local>) -> (DateTime<Local>, DateTime<Local>) {
    let today = now.date_naive();
    let week_start = today - ChronoDuration::days(today.weekday().num_days_from_monday() as i64);
    let month_start = today.with_day(1).expect("每月都有 1 日");
    let next_month = if month_start.month() == 12 {
        NaiveDate::from_ymd_opt(month_start.year() + 1, 1, 1)
    } else {
        NaiveDate::from_ymd_opt(month_start.year(), month_start.month() + 1, 1)
    }
    .expect("合法日期");
    let prev_month = if month_start.month() == 1 {
        NaiveDate::from_ymd_opt(month_start.year() - 1, 12, 1)
    } else {
        NaiveDate::from_ymd_opt(month_start.year(), month_start.month() - 1, 1)
    }
    .expect("合法日期");
    let year_start = NaiveDate::from_ymd_opt(today.year(), 1, 1).expect("合法日期");
    let (a, b) = match p {
        Period::Today => (today, today + ChronoDuration::days(1)),
        Period::Yesterday => (today - ChronoDuration::days(1), today),
        Period::ThisWeek => (week_start, week_start + ChronoDuration::days(7)),
        Period::LastWeek => (week_start - ChronoDuration::days(7), week_start),
        Period::ThisMonth => (month_start, next_month),
        Period::LastMonth => (prev_month, month_start),
        Period::ThisYear => (
            year_start,
            NaiveDate::from_ymd_opt(today.year() + 1, 1, 1).expect("合法日期"),
        ),
        Period::LastYear => (
            NaiveDate::from_ymd_opt(today.year() - 1, 1, 1).expect("合法日期"),
            year_start,
        ),
    };
    (midnight(a), midnight(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fileinfo::FileInfo;
    use std::path::PathBuf;

    fn file(name: &str, size: u64, modified: DateTime<Local>) -> FileCtx {
        let path = PathBuf::from(format!("/data/2024/{name}"));
        let (name, stem, ext) = crate::fileinfo::split_name(&path);
        FileInfo {
            path,
            name,
            stem,
            ext,
            size,
            created: None,
            modified: Some(modified),
            accessed: None,
            hidden: false,
            readonly: false,
        }
        .into()
    }

    fn cond(yaml: &str) -> Condition {
        let def: ConditionDef = serde_yaml_ng::from_str(yaml).unwrap();
        Condition::compile(&def).unwrap_or_else(|e| panic!("{e}\n{yaml}"))
    }

    fn now() -> DateTime<Local> {
        Local.with_ymd_and_hms(2026, 3, 18, 12, 0, 0).unwrap()
    }

    fn hit(c: &Condition, f: &FileCtx) -> bool {
        c.matches(f, &mut MatchCtx::new(now()))
    }

    #[test]
    fn text_operators() {
        let f = file("IMG_2034 发票.PDF", 10, now());
        assert!(hit(&cond("name: { contains: 发票 }"), &f));
        assert!(hit(&cond("name: { starts_with: img_ }"), &f));
        assert!(!hit(
            &cond("name: { starts_with: img_, case_sensitive: true }"),
            &f
        ));
        assert!(hit(&cond("ext: pdf"), &f));
        assert!(hit(&cond("extension: [.jpg, .PDF]"), &f));
        assert!(hit(&cond("ext: { not_in: [doc, xls] }"), &f));
        assert!(hit(&cond("name: '*发票*'"), &f));
        assert!(hit(&cond("name: { matches: '^IMG_(\\d+)' }"), &f));
        assert!(hit(&cond("name: { not_contains: 草稿 }"), &f));
        assert!(hit(&cond("category: 文档"), &f));
        assert!(hit(&cond("category: Documents"), &f));
        // 比较值里用变量：所在文件夹名等于修改年份
        let g = file(
            "x.txt",
            1,
            Local.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap(),
        );
        assert!(hit(&cond("parent: { eq: '{modified.year}' }"), &g));
    }

    #[test]
    fn number_size_and_exists() {
        let f = file("a.zip", 5_000_000, now());
        assert!(hit(&cond("size: { gt: 1MB, lt: 10MB }"), &f));
        assert!(hit(&cond("size: { '>=': 5000000 }"), &f));
        assert!(hit(&cond("size: { between: [4MB, 6MB] }"), &f));
        assert!(!hit(&cond("size: { lt: 1KB }"), &f));
        // 不是图片：没有宽度
        assert!(hit(&cond("image.width: { exists: false }"), &f));
        assert!(!hit(&cond("image.width: { gt: 0 }"), &f));
        assert!(hit(&cond("modified.month: 3"), &f));
        assert!(hit(&cond("modified.weekday: { lte: 5 }"), &f)); // 2026-03-18 是周三
    }

    #[test]
    fn dates() {
        let f = file(
            "a.txt",
            1,
            Local.with_ymd_and_hms(2026, 3, 2, 9, 0, 0).unwrap(),
        );
        assert!(hit(&cond("modified: { older_than: 2w }"), &f));
        assert!(!hit(&cond("modified: { newer_than: 7d }"), &f));
        assert!(hit(
            &cond("modified: { after: 2026-03-01, before: 2026-03-03 }"),
            &f
        ));
        assert!(!hit(&cond("modified: { after: 2026-03-02 }"), &f)); // 晚于 3 月 2 日 = 3 日 0 点以后
        assert!(hit(&cond("modified: 2026-03-02"), &f));
        assert!(hit(
            &cond("modified: { between: [2026-03-01, 2026-03-02] }"),
            &f
        ));
        assert!(hit(&cond("modified: { within: this_month }"), &f));
        assert!(hit(&cond("modified: { within: 今年 }"), &f));
        assert!(!hit(&cond("modified: { within: this_week }"), &f));
        assert!(!hit(&cond("created: { older_than: 1d }"), &f)); // 没有创建时间
    }

    #[test]
    fn logic_groups() {
        let f = file("报告-草稿.docx", 1, now());
        assert!(hit(
            &cond("all: [ { ext: docx }, { not: { name: { contains: 终稿 } } } ]"),
            &f
        ));
        assert!(hit(&cond("any: [ { ext: pdf }, { ext: docx } ]"), &f));
        assert!(!hit(
            &cond("none: [ { ext: pdf }, { name: { contains: 草稿 } } ]"),
            &f
        ));
        assert!(hit(
            &cond(
                "all: [ { any: [ { ext: pdf }, { ext: docx } ] }, { none: [ { size: { gt: 1GB } } ] } ]"
            ),
            &f
        ));
    }

    #[test]
    fn regex_captures() {
        let f = file("IMG_20240517_123.jpg", 1, now());
        let c = cond("name: { matches: '^img_(?P<date>\\d{8})_(\\d+)' }");
        let mut ctx = MatchCtx::new(now());
        assert!(c.matches(&f, &mut ctx));
        assert_eq!(ctx.captures["date"], "20240517");
        assert_eq!(ctx.captures["2"], "123");
    }

    #[test]
    fn compile_errors() {
        let bad = |y: &str| {
            let def: ConditionDef = serde_yaml_ng::from_str(y).unwrap();
            Condition::compile(&def).unwrap_err()
        };
        assert!(bad("nope: 1").contains("未知字段"));
        assert!(bad("size: { contains: x }").contains("不支持"));
        assert!(bad("name: { matches: '(' }").contains("正则"));
        assert!(bad("modified: { within: someday }").contains("时间段"));
        assert!(bad("category: 不存在").contains("未知类型"));
        assert!(bad("image.orientation: 斜").contains("方向"));
        assert!(bad("any: []").contains("至少"));
        assert!(serde_yaml_ng::from_str::<ConditionDef>("{ ext: pdf, name: x }").is_err());
    }
}
