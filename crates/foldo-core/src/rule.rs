//! 规则校验与编译：把 YAML 结构变成可直接执行的 `Rule`。

use crate::condition::Condition;
use crate::config::{ActionDef, MatchMode, OnConflict, RuleDef, RuleFile};
use crate::error::{Error, Result};
use crate::paths::{check_location, expand_home};
use crate::template::Template;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

pub const SUPPORTED_VERSION: u32 = 1;

#[derive(Debug)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    pub enabled: bool,
    pub locations: Vec<PathBuf>,
    pub recursive: bool,
    pub include_hidden: bool,
    pub condition: Condition,
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
}

impl RuleSet {
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|err| Error::ReadConfig {
            path: path.to_path_buf(),
            err,
        })?;
        // Windows PowerShell 5 和旧版记事本保存 UTF-8 时会在开头写入 BOM
        Self::from_yaml(text.strip_prefix('\u{feff}').unwrap_or(&text))
    }

    pub fn from_yaml(text: &str) -> Result<Self> {
        let file: RuleFile = serde_yaml_ng::from_str(text)?;
        if file.version != SUPPORTED_VERSION {
            return Err(Error::Other(format!(
                "不支持的规则文件版本 {}，当前只支持 version: {SUPPORTED_VERSION}",
                file.version
            )));
        }
        let mut seen = HashSet::new();
        let mut rules = Vec::with_capacity(file.rules.len());
        for def in file.rules {
            let rule = Rule::compile(def)?;
            if !seen.insert(rule.name.clone()) {
                return Err(Error::InvalidRule {
                    rule: rule.name,
                    msg: "规则名称重复".into(),
                });
            }
            rules.push(rule);
        }
        Ok(Self { rules })
    }

    pub fn get(&self, name: &str) -> Option<&Rule> {
        self.rules.iter().find(|r| r.name == name)
    }
}

impl Rule {
    pub fn compile(def: RuleDef) -> Result<Self> {
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

        let locations: Vec<PathBuf> = def
            .locations
            .into_vec()
            .iter()
            .map(|s| expand_home(s.trim()))
            .collect();
        if locations.is_empty() {
            return Err(invalid("locations 至少需要一个目录".into()));
        }
        for loc in &locations {
            check_location(loc, def.recursive).map_err(invalid)?;
        }

        let conditions = def
            .conditions
            .into_iter()
            .map(Condition::compile)
            .collect::<Result<Vec<_>, _>>()
            .map_err(invalid)?;
        let condition = match def.match_mode {
            // 没有条件 = 匹配位置下的所有文件
            _ if conditions.is_empty() => Condition::All(vec![]),
            MatchMode::All => Condition::All(conditions),
            MatchMode::Any => Condition::Any(conditions),
        };

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
            });
        }

        Ok(Self {
            name,
            enabled: def.enabled,
            locations,
            recursive: def.recursive,
            include_hidden: def.include_hidden,
            condition,
            actions,
        })
    }
}
