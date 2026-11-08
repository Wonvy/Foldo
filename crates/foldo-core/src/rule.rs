//! 配置校验与编译：把 YAML 结构变成可直接执行的 `RuleSet`。

use crate::condition::Condition;
use crate::config::{ActionDef, MatchMode, OnConflict, RuleDef, RuleFile};
use crate::error::{Error, Result};
use crate::paths::{check_location, expand_home, path_key};
use crate::protect::Protect;
use crate::template::Template;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const SUPPORTED_VERSION: u32 = 1;

/// 编译后的完整配置。只有用户建的规则会移动文件；没有规则命中的文件保持原位。
#[derive(Debug)]
pub struct RuleSet {
    /// 用户规则，按优先级排列。
    pub rules: Vec<Rule>,
    /// 整理范围。
    pub folders: Vec<PathBuf>,
    pub protect: Protect,
    /// 新文件缓冲期。
    pub buffer: Duration,
    /// 后台自动整理的方式。
    pub auto: crate::config::AutoMode,
    /// 整理后删除变空的文件夹。
    pub clean_empty_folders: bool,
}

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    /// 给用户看的“为什么这样整理”，如“规则「截图归档」”。
    pub reason: String,
    pub enabled: bool,
    pub locations: Vec<PathBuf>,
    pub recursive: bool,
    pub include_hidden: bool,
    pub condition: Condition,
    /// 例外：命中则这条规则不适用。
    pub exception: Option<Condition>,
    pub actions: Vec<Action>,
}

#[derive(Debug)]
pub enum Action {
    Move {
        to: Template,
        on_conflict: OnConflict,
    },
    Copy {
        to: Template,
        on_conflict: OnConflict,
    },
    Rename {
        to: Template,
        on_conflict: OnConflict,
    },
    Trash,
    Keep,
}

/// 读取配置文件的原始结构（图形界面编辑用）。
pub fn load_rule_file(path: &Path) -> Result<RuleFile> {
    let text = std::fs::read_to_string(path).map_err(|err| Error::ReadConfig {
        path: path.to_path_buf(),
        err,
    })?;
    parse_rule_file(&text)
}

pub fn parse_rule_file(text: &str) -> Result<RuleFile> {
    // Windows PowerShell 5 和旧版记事本保存 UTF-8 时会在开头写入 BOM
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let file: RuleFile = serde_yaml_ng::from_str(text)?;
    if file.version != SUPPORTED_VERSION {
        return Err(Error::Other(format!(
            "不支持的规则文件版本 {}，当前只支持 version: {SUPPORTED_VERSION}",
            file.version
        )));
    }
    Ok(file)
}

impl RuleSet {
    pub fn load(path: &Path) -> Result<Self> {
        Self::from_file(load_rule_file(path)?)
    }

    pub fn from_yaml(text: &str) -> Result<Self> {
        Self::from_file(parse_rule_file(text)?)
    }

    pub fn from_file(file: RuleFile) -> Result<Self> {
        let config_err = |msg: String| Error::Other(format!("配置无效: {msg}"));

        let mut folders: Vec<PathBuf> = Vec::new();
        let mut seen_folders = HashSet::new();
        for f in &file.folders {
            let path = expand_home(f.path.trim());
            check_location(&path, false).map_err(config_err)?;
            if seen_folders.insert(path_key(&path)) {
                folders.push(path);
            }
        }

        let protect = Protect::compile(&file.protect).map_err(config_err)?;
        let buffer = file.settings.buffer.map(|s| s.0).unwrap_or_default();

        let mut names = HashSet::new();
        let mut rules = Vec::with_capacity(file.rules.len());
        for def in file.rules {
            let rule = Rule::compile(def, &folders)?;
            if !names.insert(rule.name.clone()) {
                return Err(Error::InvalidRule {
                    rule: rule.name,
                    msg: "规则名称重复".into(),
                });
            }
            rules.push(rule);
        }

        Ok(Self {
            rules,
            folders,
            protect,
            buffer,
            auto: file.settings.auto,
            clean_empty_folders: !file.settings.keep_empty_folders,
        })
    }

    pub fn get(&self, name: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.name == name)
    }
}

impl Rule {
    /// `default_locations`：规则没写 `locations` 时使用（即整理范围里的全部文件夹）。
    pub fn compile(def: RuleDef, default_locations: &[PathBuf]) -> Result<Self> {
        let name = def.name.trim().to_string();
        let invalid = |msg: String| Error::InvalidRule {
            rule: if name.is_empty() {
                "(未命名)".into()
            } else {
                name.clone()
            },
            msg,
        };
        if name.is_empty() {
            return Err(invalid("name 不能为空".into()));
        }

        let locations: Vec<PathBuf> = match def.locations {
            Some(locs) => locs
                .into_vec()
                .iter()
                .map(|s| expand_home(s.trim()))
                .collect(),
            None => default_locations.to_vec(),
        };
        if locations.is_empty() {
            return Err(invalid(
                "没有指定 locations，也没有设置整理文件夹（folders）".into(),
            ));
        }
        for loc in &locations {
            check_location(loc, def.recursive).map_err(invalid)?;
        }

        let compile_list = |list: &[crate::config::ConditionDef]| {
            list.iter()
                .map(Condition::compile)
                .collect::<Result<Vec<_>, _>>()
                .map_err(invalid)
        };
        // `conditions`（列表 + match）和 `when`（条件树）可以同时写，两者都要满足
        let mut parts = Vec::new();
        let conditions = compile_list(&def.conditions)?;
        if !conditions.is_empty() {
            parts.push(match def.match_mode {
                MatchMode::All => Condition::All(conditions),
                MatchMode::Any => Condition::Any(conditions),
            });
        }
        if let Some(w) = &def.when {
            parts.push(Condition::compile(w).map_err(invalid)?);
        }
        // 没有条件 = 匹配位置下的所有文件
        let condition = match parts.len() {
            0 => Condition::All(vec![]),
            1 => parts.pop().expect("恰好一个"),
            _ => Condition::All(parts),
        };
        let exceptions = compile_list(&def.exceptions)?;
        let exception = (!exceptions.is_empty()).then_some(Condition::Any(exceptions));

        if def.actions.is_empty() {
            return Err(invalid("actions 至少需要一个动作".into()));
        }
        let last = def.actions.len() - 1;
        let mut actions = Vec::with_capacity(def.actions.len());
        for (i, a) in def.actions.into_iter().enumerate() {
            let tpl = |s: String| Template::parse(&s).map_err(invalid);
            actions.push(match a {
                ActionDef::Move(d) => {
                    let (to, on_conflict) = d.into_parts();
                    Action::Move {
                        to: tpl(to)?,
                        on_conflict,
                    }
                }
                ActionDef::Copy(d) => {
                    let (to, on_conflict) = d.into_parts();
                    Action::Copy {
                        to: tpl(to)?,
                        on_conflict,
                    }
                }
                ActionDef::Rename(d) => {
                    let (to, on_conflict) = d.into_parts();
                    Action::Rename {
                        to: tpl(to)?,
                        on_conflict,
                    }
                }
                ActionDef::Trash => {
                    if i != last {
                        return Err(invalid("trash 必须是最后一个动作".into()));
                    }
                    Action::Trash
                }
                ActionDef::Keep => {
                    if last != 0 {
                        return Err(invalid("keep（保持原位）不能和其他动作一起使用".into()));
                    }
                    Action::Keep
                }
            });
        }

        Ok(Self {
            reason: format!("规则「{name}」"),
            name,
            enabled: def.enabled,
            locations,
            recursive: def.recursive,
            include_hidden: def.include_hidden,
            condition,
            exception,
            actions,
        })
    }
}
