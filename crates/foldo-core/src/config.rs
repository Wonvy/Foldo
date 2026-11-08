//! 配置文件的原始结构：既能从 YAML 读取，也能写回 YAML（图形界面的表单保存时用）。
//! 校验和编译在 `rule.rs` 里完成。

use crate::units::Span;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
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

fn default_true() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleFile {
    pub version: u32,
    /// 整理的文件夹，每个文件夹指定用哪些规则。
    /// 只有规则命中的文件才会被移动，其余文件保持原位。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub folders: Vec<FolderDef>,
    /// 固定保留：路径（含 `/`、`\` 或以 `~` 开头）或文件名通配符，命中的项目永远不会被移动。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub protect: Vec<String>,
    #[serde(default, skip_serializing_if = "SettingsDef::is_default")]
    pub settings: SettingsDef,
    /// 规则里的枚举（条件、动作）用 `move: ...` 这种单键 map 写法，而不是 YAML 标签 `!move`。
    #[serde(default, with = "serde_yaml_ng::with::singleton_map_recursive")]
    pub rules: Vec<RuleDef>,
}

impl RuleFile {
    /// 写成 YAML 文本。
    pub fn to_yaml(&self) -> Result<String, serde_yaml_ng::Error> {
        serde_yaml_ng::to_string(self)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FolderDef {
    pub path: String,
    /// 是否连子文件夹里的文件一起整理。
    #[serde(default, skip_serializing_if = "is_false")]
    pub recursive: bool,
    /// 是否处理隐藏文件。
    #[serde(default, skip_serializing_if = "is_false")]
    pub include_hidden: bool,
    /// 用在这个文件夹的规则名称。按本列表顺序执行，第一条命中的规则生效。
    /// 没有写（或为空）时这个文件夹不整理任何文件。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsDef {
    /// 按内置类型 ID 覆盖扩展名列表；未配置的类型仍使用默认值。
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub category_extensions: std::collections::BTreeMap<String, Vec<String>>,
    /// 新文件缓冲期：最近这段时间内新增或修改的文件暂不整理。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub buffer: Option<Span>,
    /// 自动整理：off（默认）/ notify（发现可整理的文件时提醒）/ run（直接整理）。
    #[serde(default, skip_serializing_if = "AutoMode::is_off")]
    pub auto: AutoMode,
    /// 保留整理后变空的文件夹。默认会删除它们（撤销时会自动重建）。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub keep_empty_folders: bool,
}

impl SettingsDef {
    fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

/// 后台自动整理的方式。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AutoMode {
    /// 不自动整理
    #[default]
    Off,
    /// 发现可以整理的文件时提醒，由用户确认
    Notify,
    /// 直接整理（不可撤销的“移到回收站”除外，仍需确认）
    Run,
}

impl AutoMode {
    fn is_off(&self) -> bool {
        *self == AutoMode::Off
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuleDef {
    pub name: String,
    #[serde(default = "default_true", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// 处理对象：file（默认）或 folder（整理文件夹第一层的子文件夹）。
    #[serde(default, skip_serializing_if = "Target::is_file")]
    pub target: Target,
    /// 旧版（version 1）写法：规则自己的生效文件夹。读取时自动移到 `folders` 里，不再写出。
    #[serde(default, skip_serializing)]
    pub locations: Option<OneOrMany<String>>,
    /// 旧版写法，读取时移到对应文件夹的 `recursive`。
    #[serde(default, skip_serializing)]
    pub recursive: bool,
    /// 旧版写法，读取时移到对应文件夹的 `include_hidden`。
    #[serde(default, skip_serializing)]
    pub include_hidden: bool,
    #[serde(rename = "match", default, skip_serializing_if = "MatchMode::is_all")]
    pub match_mode: MatchMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<ConditionDef>,
    /// 条件树（可任意嵌套 all / any / none / not）。与 `conditions` 同时存在时两者都要满足。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<ConditionDef>,
    /// 例外：命中其中任意一条，这条规则就不适用。
    #[serde(rename = "except", default, skip_serializing_if = "Vec::is_empty")]
    pub exceptions: Vec<ConditionDef>,
    pub actions: Vec<ActionDef>,
}

/// 规则处理的对象。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Target {
    #[default]
    File,
    /// 整理文件夹第一层的子文件夹（整个文件夹一起处理）
    Folder,
}

impl Target {
    pub fn is_file(&self) -> bool {
        *self == Target::File
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MatchMode {
    #[default]
    All,
    Any,
}

impl MatchMode {
    fn is_all(&self) -> bool {
        *self == MatchMode::All
    }
}

/// 一个条件：只有一个键的 map。
///
/// - 键是字段名时为比较，如 `image.width: { ">=": 3000 }`、`name: { matches: '^IMG_' }`、`category: 图片`
/// - 键是 `all` / `any` / `none` 时为条件组（与 / 或 / 都不），值是条件列表
/// - 键是 `not` 时对单个条件取反
///
/// 值原样保存（`serde_json::Value`），在 `condition.rs` 里按字段类型编译；写回 YAML 时保持用户原来的写法。
#[derive(Debug, Clone, PartialEq)]
pub struct ConditionDef {
    pub key: String,
    pub value: serde_json::Value,
}

impl Serialize for ConditionDef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(Some(1))?;
        m.serialize_entry(&self.key, &self.value)?;
        m.end()
    }
}

impl<'de> Deserialize<'de> for ConditionDef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let map = std::collections::BTreeMap::<String, serde_json::Value>::deserialize(d)?;
        if map.len() != 1 {
            let keys: Vec<&str> = map.keys().map(String::as_str).collect();
            return Err(serde::de::Error::custom(format!(
                "每个条件只能有一个键（这里有：{}）。多个条件请写成列表，或放进 all / any 里",
                keys.join("、")
            )));
        }
        let (key, value) = map.into_iter().next().expect("恰好一个键");
        Ok(Self { key, value })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionDef {
    /// 移动到目标文件夹，文件名不变。
    Move(DestDef),
    /// 复制到目标文件夹，后续动作继续作用于原文件。
    Copy(DestDef),
    /// 在原文件夹内改名。
    Rename(DestDef),
    /// 解压 zip 到文件夹（相对路径以文件所在文件夹为准，如 `{stem}/`）。原文件保留，后续动作继续作用于它。
    Extract(DestDef),
    /// 压缩成 zip（如 `{stem}.zip`，相对文件所在文件夹）。原文件保留。
    Compress(DestDef),
    /// 在目标文件夹里创建指向这个文件的快捷方式。原文件保留。
    Shortcut(DestDef),
    /// 移入系统回收站，必须是最后一个动作。
    Trash,
    /// 保持原位：命中这条规则的文件不移动（用于“这类文件别动”）。只能单独使用。
    Keep,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DestDef {
    Short(String),
    Full(DestFull),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// 目标已存在时怎么办。默认保留两份，绝不自动覆盖。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OnConflict {
    /// 保留两份：新文件自动改名为 `name (1).ext`（默认）。
    #[default]
    Rename,
    /// 跳过这个文件。
    Skip,
    /// 把已有文件移入回收站后再放入（只能在 YAML 里写，图形界面不提供）。
    Overwrite,
}
