//! 规则文件的原始结构，由 YAML 直接反序列化得到；校验和编译在 `rule.rs` 里完成。

use crate::units::{ByteSize, Span};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum OneOrMany<T> {
    One(T),
    Many(Vec<T>),
}

impl<T> OneOrMany<T> {
    pub fn into_vec(self) -> Vec<T> {
        match self {
            Self::One(t) => vec![t],
            Self::Many(v) => v,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    pub version: u32,
    /// 规则里的枚举（条件、动作）用 `move: ...` 这种单键 map 写法，而不是 YAML 标签 `!move`。
    #[serde(default, with = "serde_yaml_ng::with::singleton_map_recursive")]
    pub rules: Vec<RuleDef>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDef {
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub locations: OneOrMany<String>,
    /// 是否扫描子文件夹。
    #[serde(default)]
    pub recursive: bool,
    /// 是否处理隐藏文件。
    #[serde(default)]
    pub include_hidden: bool,
    #[serde(rename = "match", default)]
    pub match_mode: MatchMode,
    #[serde(default)]
    pub conditions: Vec<ConditionDef>,
    pub actions: Vec<ActionDef>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    All,
    Any,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionDef {
    /// `extension: [jpg, png]` 或 `extension: pdf`，不区分大小写。
    Extension(OneOrMany<String>),
    /// `name: "*.pdf"`（glob 简写）或 `name: { regex: "...", case_sensitive: true }`
    Name(NameDef),
    Size(SizeDef),
    Created(AgeDef),
    Modified(AgeDef),
    Accessed(AgeDef),
    Hidden(bool),
    All(Vec<ConditionDef>),
    Any(Vec<ConditionDef>),
    Not(Box<ConditionDef>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum NameDef {
    Glob(String),
    Full(NameFull),
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NameFull {
    pub glob: Option<String>,
    pub regex: Option<String>,
    pub contains: Option<String>,
    pub starts_with: Option<String>,
    pub ends_with: Option<String>,
    #[serde(default)]
    pub case_sensitive: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SizeDef {
    pub gt: Option<ByteSize>,
    pub gte: Option<ByteSize>,
    pub lt: Option<ByteSize>,
    pub lte: Option<ByteSize>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgeDef {
    /// 距今超过多久，例如 `30d`。
    pub older_than: Option<Span>,
    /// 距今不到多久，例如 `2h`。
    pub newer_than: Option<Span>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionDef {
    /// 移动到目标文件夹，文件名不变。
    Move(DestDef),
    /// 复制到目标文件夹，后续动作继续作用于原文件。
    Copy(DestDef),
    /// 在原文件夹内改名。
    Rename(DestDef),
    /// 移入系统回收站，必须是最后一个动作。
    Trash,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum DestDef {
    Short(String),
    Full(DestFull),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DestFull {
    pub to: String,
    #[serde(default)]
    pub on_conflict: OnConflict,
}

impl DestDef {
    pub fn into_parts(self) -> (String, OnConflict) {
        match self {
            Self::Short(to) => (to, OnConflict::default()),
            Self::Full(f) => (f.to, f.on_conflict),
        }
    }
}

/// 目标已存在时怎么办。
#[derive(Debug, Clone, Copy, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnConflict {
    /// 自动改名为 `name (1).ext`（默认）。
    #[default]
    Rename,
    /// 跳过这个文件。
    Skip,
    /// 把已有文件移入回收站后再放入。
    Overwrite,
}
