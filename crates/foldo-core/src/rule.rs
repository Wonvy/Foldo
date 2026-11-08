//! 配置校验与编译：把 YAML 结构变成可直接执行的 `RuleSet`。

use crate::condition::Condition;
use crate::config::{ActionDef, FolderDef, MatchMode, OnConflict, RuleDef, RuleFile, Target};
use crate::error::{Error, Result};
use crate::paths::{check_location, expand_home, path_key};
use crate::protect::Protect;
use crate::template::Template;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// 当前的配置版本。version 1（规则自己写生效文件夹）读取时会自动迁移成 version 2（文件夹指定规则）。
pub const SUPPORTED_VERSION: u32 = 2;

/// 编译后的完整配置。只有用户建的规则会移动文件；没有规则命中的文件保持原位。
#[derive(Debug)]
pub struct RuleSet {
    pub categories: std::sync::Arc<crate::categories::CategoryMap>,
    /// 用户规则，按优先级排列。
    pub rules: Vec<Rule>,
    /// 整理的文件夹，以及各自使用的规则。
    pub folders: Vec<Folder>,
    pub protect: Protect,
    /// 新文件缓冲期。
    pub buffer: Duration,
    /// 后台自动整理的方式。
    pub auto: crate::config::AutoMode,
    /// 整理后删除变空的文件夹。
    pub clean_empty_folders: bool,
}

#[derive(Debug, Clone)]
pub struct Folder {
    pub path: PathBuf,
    pub recursive: bool,
    pub include_hidden: bool,
    /// 使用的规则在 `RuleSet::rules` 中的位置，保持文件夹配置的优先级顺序。
    pub rules: Vec<usize>,
}

impl Folder {
    /// 这个文件夹的整理范围是否包含 `file`。
    pub fn covers(&self, file: &Path) -> bool {
        let Some(parent) = file.parent() else {
            return false;
        };
        let (p, f) = (path_key(parent), path_key(&self.path));
        p == f || (self.recursive && p.starts_with(&f) && p[f.len()..].starts_with(['/', '\\']))
    }
}

#[derive(Debug)]
pub struct Rule {
    pub name: String,
    /// 给用户看的“为什么这样整理”，如“规则「截图归档」”。
    pub reason: String,
    pub enabled: bool,
    /// 处理文件还是文件夹。
    pub target: Target,
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
    Extract {
        to: Template,
        on_conflict: OnConflict,
    },
    Compress {
        to: Template,
        on_conflict: OnConflict,
    },
    Shortcut {
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
    let mut file: RuleFile = serde_yaml_ng::from_str(text)?;
    migrate(&mut file)?;
    Ok(file)
}

/// 把旧写法转换成当前写法（可重复调用）：
/// - 规则里的 `locations` / `recursive` / `include_hidden` 移到对应的文件夹上，规则加入这些文件夹
/// - version 1 中没写 `locations` 的规则原本对全部文件夹生效，迁移后加入每个原有文件夹
pub fn migrate(file: &mut RuleFile) -> Result<()> {
    if file.version == 0 || file.version > SUPPORTED_VERSION {
        return Err(Error::Other(crate::t!(
            "不支持的规则文件版本 {a0}，当前支持 version: 1 和 {SUPPORTED_VERSION}",
            a0 = file.version,
            SUPPORTED_VERSION = SUPPORTED_VERSION
        )));
    }
    let legacy = file.version < 2;
    let RuleFile { folders, rules, .. } = file;
    let original = folders.len();
    if legacy {
        for f in folders.iter_mut() {
            f.rules.get_or_insert_with(Vec::new);
        }
    }
    for r in rules.iter_mut() {
        let locations = r.locations.take();
        let recursive = std::mem::take(&mut r.recursive);
        let hidden = std::mem::take(&mut r.include_hidden);
        let targets: Vec<usize> = match locations {
            Some(locs) => locs
                .into_vec()
                .iter()
                .map(|loc| folder_index(folders, loc))
                .collect(),
            None if legacy => (0..original).collect(),
            None => Vec::new(),
        };
        for i in targets {
            let f = &mut folders[i];
            f.recursive |= recursive;
            f.include_hidden |= hidden;
            let list = f.rules.get_or_insert_with(Vec::new);
            if !list.contains(&r.name) {
                list.push(r.name.clone());
            }
        }
    }
    file.version = SUPPORTED_VERSION;
    Ok(())
}

fn folder_index(folders: &mut Vec<FolderDef>, loc: &str) -> usize {
    let key = path_key(&expand_home(loc.trim()));
    if let Some(i) = folders
        .iter()
        .position(|f| path_key(&expand_home(f.path.trim())) == key)
    {
        return i;
    }
    folders.push(FolderDef {
        path: loc.trim().to_string(),
        rules: Some(Vec::new()),
        ..Default::default()
    });
    folders.len() - 1
}

impl RuleSet {
    pub fn load(path: &Path) -> Result<Self> {
        Self::from_file(load_rule_file(path)?)
    }

    pub fn from_yaml(text: &str) -> Result<Self> {
        Self::from_file(parse_rule_file(text)?)
    }

    pub fn from_file(mut file: RuleFile) -> Result<Self> {
        let config_err = |msg: String| Error::Other(crate::t!("配置无效: {msg}", msg = msg));
        migrate(&mut file)?;

        let categories = std::sync::Arc::new(
            crate::categories::CategoryMap::compile(&file.settings.category_extensions)
                .map_err(config_err)?,
        );

        let protect = Protect::compile(&file.protect).map_err(config_err)?;
        let buffer = file.settings.buffer.map(|s| s.0).unwrap_or_default();

        let mut index: HashMap<String, usize> = HashMap::new();
        let mut rules = Vec::with_capacity(file.rules.len());
        for def in file.rules {
            let rule = Rule::compile(def)?;
            if index.insert(rule.name.clone(), rules.len()).is_some() {
                return Err(Error::InvalidRule {
                    rule: rule.name,
                    msg: crate::t!("规则名称重复"),
                });
            }
            rules.push(rule);
        }

        // 同一个文件夹写了多次时合并
        let mut folders: Vec<Folder> = Vec::new();
        let mut by_key: HashMap<String, usize> = HashMap::new();
        for f in &file.folders {
            let path = expand_home(f.path.trim());
            let mut used = Vec::new();
            for name in f.rules.iter().flatten() {
                let i = *index.get(name.trim()).ok_or_else(|| {
                    config_err(crate::t!(
                        "文件夹 {a0} 使用了不存在的规则「{name}」",
                        a0 = crate::paths::display(&path),
                        name = name
                    ))
                })?;
                used.push(i);
            }
            let folder = match by_key.get(&path_key(&path)) {
                Some(&i) => &mut folders[i],
                None => {
                    by_key.insert(path_key(&path), folders.len());
                    folders.push(Folder {
                        path,
                        recursive: false,
                        include_hidden: false,
                        rules: Vec::new(),
                    });
                    folders.last_mut().expect("刚放入")
                }
            };
            folder.recursive |= f.recursive;
            folder.include_hidden |= f.include_hidden;
            for i in used {
                if !folder.rules.contains(&i) {
                    folder.rules.push(i);
                }
            }
        }
        for f in &folders {
            check_location(&f.path, f.recursive).map_err(config_err)?;
        }

        Ok(Self {
            categories,
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

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.rules.iter().position(|r| r.name == name)
    }

    /// 使用某条规则的文件夹。
    pub fn folders_using(&self, rule: usize) -> impl Iterator<Item = &Folder> {
        self.folders.iter().filter(move |f| f.rules.contains(&rule))
    }
}

impl Rule {
    pub fn compile(def: RuleDef) -> Result<Self> {
        let name = def.name.trim().to_string();
        let invalid = |msg: String| Error::InvalidRule {
            rule: if name.is_empty() {
                crate::t!("(未命名)")
            } else {
                name.clone()
            },
            msg,
        };
        if name.is_empty() {
            return Err(invalid(crate::t!("name 不能为空")));
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
            return Err(invalid(crate::t!("actions 至少需要一个动作")));
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
                ActionDef::Extract(_) if def.target == Target::Folder => {
                    return Err(invalid(crate::t!("解压只能用于文件，不能用于文件夹")));
                }
                ActionDef::Extract(d) => {
                    let (to, on_conflict) = d.into_parts();
                    Action::Extract {
                        to: tpl(to)?,
                        on_conflict,
                    }
                }
                ActionDef::Compress(d) => {
                    let (to, on_conflict) = d.into_parts();
                    Action::Compress {
                        to: tpl(to)?,
                        on_conflict,
                    }
                }
                ActionDef::Shortcut(d) => {
                    let (to, on_conflict) = d.into_parts();
                    Action::Shortcut {
                        to: tpl(to)?,
                        on_conflict,
                    }
                }
                ActionDef::Trash => {
                    if i != last {
                        return Err(invalid(crate::t!("trash 必须是最后一个动作")));
                    }
                    Action::Trash
                }
                ActionDef::Keep => {
                    if last != 0 {
                        return Err(invalid(crate::t!("keep（保持原位）不能和其他动作一起使用")));
                    }
                    Action::Keep
                }
            });
        }

        Ok(Self {
            reason: crate::t!("规则「{name}」", name = name),
            name,
            enabled: def.enabled,
            target: def.target,
            condition,
            exception,
            actions,
        })
    }
}
