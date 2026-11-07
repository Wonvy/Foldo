//! 编译后的匹配条件。

use crate::config::{AgeDef, ConditionDef, NameDef, NameFull, SizeDef};
use crate::fileinfo::FileInfo;
use chrono::{DateTime, Local};
use globset::{GlobBuilder, GlobMatcher};
use regex::{Regex, RegexBuilder};
use std::collections::BTreeMap;
use std::time::Duration;

/// 匹配过程中的上下文；正则的捕获组会存进 `captures`，供路径模板里的 `{regex.1}` 使用。
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

#[derive(Debug, Clone, Copy)]
pub enum TimeField {
    Created,
    Modified,
    Accessed,
}

#[derive(Debug)]
pub enum Condition {
    /// 小写、不带点。
    Extension(Vec<String>),
    Name(NameMatcher),
    Size {
        gt: Option<u64>,
        gte: Option<u64>,
        lt: Option<u64>,
        lte: Option<u64>,
    },
    Age {
        field: TimeField,
        older_than: Option<Duration>,
        newer_than: Option<Duration>,
    },
    Hidden(bool),
    All(Vec<Condition>),
    Any(Vec<Condition>),
    Not(Box<Condition>),
}

#[derive(Debug)]
pub struct NameMatcher {
    glob: Option<GlobMatcher>,
    regex: Option<Regex>,
    contains: Option<String>,
    starts_with: Option<String>,
    ends_with: Option<String>,
    case_sensitive: bool,
}

impl Condition {
    pub fn compile(def: ConditionDef) -> Result<Self, String> {
        Ok(match def {
            ConditionDef::Extension(list) => {
                let exts: Vec<String> = list
                    .into_vec()
                    .into_iter()
                    .map(|e| e.trim().trim_start_matches('.').to_lowercase())
                    .collect();
                if exts.iter().any(|e| e.is_empty()) {
                    return Err("extension 不能为空字符串".into());
                }
                Condition::Extension(exts)
            }
            ConditionDef::Name(NameDef::Glob(g)) => {
                Condition::Name(NameMatcher::compile(NameFull {
                    glob: Some(g),
                    ..Default::default()
                })?)
            }
            ConditionDef::Name(NameDef::Full(n)) => Condition::Name(NameMatcher::compile(n)?),
            ConditionDef::Size(SizeDef { gt, gte, lt, lte }) => {
                if gt.is_none() && gte.is_none() && lt.is_none() && lte.is_none() {
                    return Err("size 至少需要 gt / gte / lt / lte 之一".into());
                }
                Condition::Size {
                    gt: gt.map(|b| b.0),
                    gte: gte.map(|b| b.0),
                    lt: lt.map(|b| b.0),
                    lte: lte.map(|b| b.0),
                }
            }
            ConditionDef::Created(a) => age(TimeField::Created, a)?,
            ConditionDef::Modified(a) => age(TimeField::Modified, a)?,
            ConditionDef::Accessed(a) => age(TimeField::Accessed, a)?,
            ConditionDef::Hidden(h) => Condition::Hidden(h),
            ConditionDef::All(list) => Condition::All(compile_all(list)?),
            ConditionDef::Any(list) => {
                if list.is_empty() {
                    return Err("any 至少需要一个子条件".into());
                }
                Condition::Any(compile_all(list)?)
            }
            ConditionDef::Not(inner) => Condition::Not(Box::new(Condition::compile(*inner)?)),
        })
    }

    pub fn matches(&self, f: &FileInfo, ctx: &mut MatchCtx) -> bool {
        match self {
            Condition::Extension(exts) => {
                let ext = f.ext.to_lowercase();
                exts.contains(&ext)
            }
            Condition::Name(m) => m.matches(&f.name, ctx),
            Condition::Size { gt, gte, lt, lte } => {
                let s = f.size;
                gt.is_none_or(|v| s > v)
                    && gte.is_none_or(|v| s >= v)
                    && lt.is_none_or(|v| s < v)
                    && lte.is_none_or(|v| s <= v)
            }
            Condition::Age {
                field,
                older_than,
                newer_than,
            } => {
                let t = match field {
                    TimeField::Created => f.created,
                    TimeField::Modified => f.modified,
                    TimeField::Accessed => f.accessed,
                };
                // 有些文件系统拿不到创建时间，此时条件不成立。
                let Some(t) = t else { return false };
                let age = (ctx.now - t).to_std().unwrap_or(Duration::ZERO);
                older_than.is_none_or(|d| age >= d) && newer_than.is_none_or(|d| age < d)
            }
            Condition::Hidden(h) => f.hidden == *h,
            Condition::All(list) => list.iter().all(|c| c.matches(f, ctx)),
            Condition::Any(list) => list.iter().any(|c| c.matches(f, ctx)),
            Condition::Not(c) => !c.matches(f, ctx),
        }
    }
}

fn compile_all(list: Vec<ConditionDef>) -> Result<Vec<Condition>, String> {
    list.into_iter().map(Condition::compile).collect()
}

fn age(field: TimeField, a: AgeDef) -> Result<Condition, String> {
    if a.older_than.is_none() && a.newer_than.is_none() {
        return Err("时间条件至少需要 older_than 或 newer_than".into());
    }
    Ok(Condition::Age {
        field,
        older_than: a.older_than.map(|s| s.0),
        newer_than: a.newer_than.map(|s| s.0),
    })
}

impl NameMatcher {
    fn compile(n: NameFull) -> Result<Self, String> {
        let cs = n.case_sensitive;
        let fold = |s: Option<String>| s.map(|v| if cs { v } else { v.to_lowercase() });
        let glob = n
            .glob
            .map(|g| {
                GlobBuilder::new(&g)
                    .case_insensitive(!cs)
                    .build()
                    .map(|g| g.compile_matcher())
                    .map_err(|e| format!("glob「{g}」无效: {e}"))
            })
            .transpose()?;
        let regex = n
            .regex
            .map(|r| {
                RegexBuilder::new(&r)
                    .case_insensitive(!cs)
                    .build()
                    .map_err(|e| format!("正则「{r}」无效: {e}"))
            })
            .transpose()?;
        let m = Self {
            glob,
            regex,
            contains: fold(n.contains),
            starts_with: fold(n.starts_with),
            ends_with: fold(n.ends_with),
            case_sensitive: cs,
        };
        if m.glob.is_none()
            && m.regex.is_none()
            && m.contains.is_none()
            && m.starts_with.is_none()
            && m.ends_with.is_none()
        {
            return Err(
                "name 至少需要 glob / regex / contains / starts_with / ends_with 之一".into(),
            );
        }
        Ok(m)
    }

    fn matches(&self, name: &str, ctx: &mut MatchCtx) -> bool {
        let folded;
        let cmp: &str = if self.case_sensitive {
            name
        } else {
            folded = name.to_lowercase();
            &folded
        };
        if let Some(g) = &self.glob
            && !g.is_match(name)
        {
            return false;
        }
        if self.contains.as_deref().is_some_and(|s| !cmp.contains(s))
            || self
                .starts_with
                .as_deref()
                .is_some_and(|s| !cmp.starts_with(s))
            || self.ends_with.as_deref().is_some_and(|s| !cmp.ends_with(s))
        {
            return false;
        }
        if let Some(re) = &self.regex {
            let Some(caps) = re.captures(name) else {
                return false;
            };
            for (i, m) in caps.iter().enumerate() {
                if let Some(m) = m {
                    ctx.captures.insert(i.to_string(), m.as_str().to_string());
                }
            }
            for gname in re.capture_names().flatten() {
                if let Some(m) = caps.name(gname) {
                    ctx.captures
                        .insert(gname.to_string(), m.as_str().to_string());
                }
            }
        }
        true
    }
}
