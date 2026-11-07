//! Foldo 核心引擎：规则解析 → 扫描 → 匹配 → 生成计划 → 执行 → 记录日志 → 撤销。
//!
//! 这个 crate 不依赖任何 UI，CLI 和以后的 Tauri GUI 都只是它外面的一层。

pub mod condition;
pub mod config;
pub mod error;
pub mod exec;
pub mod fileinfo;
pub mod fsops;
pub mod journal;
pub mod paths;
pub mod plan;
pub mod rule;
pub mod scan;
pub mod template;
pub mod undo;
pub mod units;

/// 内置的示例规则，`foldo init` 和图形界面的“生成示例规则”都用它。
pub const EXAMPLE_RULES: &str = include_str!("../../../examples/rules.yaml");

pub use error::{Error, Result};
pub use exec::{ExecReport, execute};
pub use journal::Journal;
pub use plan::{Op, OpKind, Plan, build_plan, build_plan_at};
pub use rule::{Rule, RuleSet};
pub use undo::{UndoReport, undo_batch};
