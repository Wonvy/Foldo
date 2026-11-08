use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use foldo_core::journal::status;
use foldo_core::paths::{default_data_dir, default_rules_path, display};
use foldo_core::{
    EXAMPLE_RULES, Journal, OpKind, Plan, RuleSet, SkipKind, build_plan, execute, undo_batch,
};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

/// 单次操作数超过这个值时额外提醒。
const LARGE_BATCH: usize = 500;

#[derive(Parser)]
#[command(name = "foldo", version, about = "Foldo — 按规则整理文件夹")]
struct Cli {
    /// 规则文件路径（默认：系统配置目录下的 foldo/rules.yaml）
    #[arg(short, long, global = true, env = "FOLDO_RULES")]
    rules: Option<PathBuf>,

    /// 数据目录，存放操作日志（默认：系统数据目录下的 foldo）
    #[arg(long, global = true, env = "FOLDO_HOME")]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// 生成一份示例规则文件
    Init {
        /// 覆盖已有的规则文件
        #[arg(long)]
        force: bool,
    },
    /// 校验规则文件
    Check,
    /// 预览将要执行的操作（不会改动任何文件）
    Plan {
        /// 只运行指定名称的规则
        #[arg(long)]
        rule: Option<String>,
        /// 以 JSON 输出
        #[arg(long)]
        json: bool,
    },
    /// 按规则整理文件
    Run {
        /// 只运行指定名称的规则
        #[arg(long)]
        rule: Option<String>,
        /// 跳过确认
        #[arg(short, long)]
        yes: bool,
    },
    /// 查看执行历史
    History {
        #[arg(short, long, default_value_t = 20)]
        limit: usize,
    },
    /// 查看一个文件的全部属性（写规则时参考字段名和取值）
    Inspect {
        /// 文件路径
        path: PathBuf,
        /// 同时列出读不到的字段
        #[arg(long)]
        all: bool,
    },
    /// 撤销一次执行（默认撤销最近一次）
    Undo {
        /// 批次编号，见 `foldo history`
        batch: Option<i64>,
        /// 跳过确认
        #[arg(short, long)]
        yes: bool,
    },
}

fn main() {
    if let Err(e) = run() {
        eprintln!("错误: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let rules_path = cli.rules.clone().unwrap_or_else(default_rules_path);
    let data_dir = cli.data_dir.clone().unwrap_or_else(default_data_dir);

    match cli.cmd {
        Cmd::Init { force } => init(&rules_path, force),
        Cmd::Check => check(&rules_path),
        Cmd::Plan { rule, json } => {
            let rules = load(&rules_path)?;
            let plan = build_plan(&rules, rule.as_deref())?;
            if json {
                println!("{}", serde_json::to_string_pretty(&plan)?);
            } else {
                print_plan(&plan);
            }
            Ok(())
        }
        Cmd::Run { rule, yes } => {
            let rules = load(&rules_path)?;
            let plan = build_plan(&rules, rule.as_deref())?;
            print_plan(&plan);
            if plan.is_empty() {
                return Ok(());
            }
            if plan.ops.len() > LARGE_BATCH {
                println!(
                    "\n⚠ 本次操作数量较多（{} 项），请仔细核对。",
                    plan.ops.len()
                );
            }
            if !yes && !confirm(&format!("\n执行以上 {} 项操作？", plan.ops.len()))? {
                println!("已取消。");
                return Ok(());
            }
            let journal = Journal::open_in(&data_dir)?;
            let report = execute(&plan, &journal, "cli")?;
            println!(
                "\n✓ 批次 #{}：完成 {} 项，跳过 {} 项，失败 {} 项。",
                report.batch_id,
                report.done,
                report.skipped.len(),
                report.failed.len()
            );
            for (op, err) in &report.skipped {
                println!("  ↷ {}  ({err})", display(&op.src));
            }
            for (op, err) in &report.failed {
                println!("  ✗ {}  ({err})", display(&op.src));
            }
            if report.done > 0 {
                println!("如需撤销：foldo undo {}", report.batch_id);
            }
            if !report.failed.is_empty() {
                bail!("有 {} 项操作失败", report.failed.len());
            }
            Ok(())
        }
        Cmd::History { limit } => history(&data_dir, limit),
        Cmd::Undo { batch, yes } => undo(&data_dir, batch, yes),
        Cmd::Inspect { path, all } => inspect(&path, all, &rules_path),
    }
}

fn load(path: &Path) -> Result<RuleSet> {
    if !path.exists() {
        bail!(
            "规则文件不存在：{}\n可以先运行 `foldo init` 生成示例规则，或用 --rules 指定路径。",
            display(path)
        );
    }
    Ok(RuleSet::load(path)?)
}

fn init(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!("规则文件已存在：{}（使用 --force 覆盖）", display(path));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("无法创建目录 {}", display(parent)))?;
    }
    std::fs::write(path, EXAMPLE_RULES).with_context(|| format!("无法写入 {}", display(path)))?;
    println!("✓ 已生成示例规则：{}", display(path));
    println!("编辑后运行 `foldo plan` 预览效果。");
    Ok(())
}

fn check(path: &Path) -> Result<()> {
    let rules = load(path)?;
    let enabled = rules.rules.iter().filter(|r| r.enabled).count();
    println!(
        "✓ 规则文件有效：{}，共 {} 条规则，{} 条启用",
        display(path),
        rules.rules.len(),
        enabled
    );
    for r in &rules.rules {
        println!("  {} {}", if r.enabled { "●" } else { "○" }, r.name);
    }
    println!("整理文件夹：");
    for f in &rules.folders {
        let names: Vec<&str> = f
            .rules
            .iter()
            .map(|&i| rules.rules[i].name.as_str())
            .collect();
        println!(
            "  {}{}  →  {}",
            display(&f.path),
            if f.recursive {
                "（含子文件夹）"
            } else {
                ""
            },
            if names.is_empty() {
                "（没有使用规则）".to_string()
            } else {
                names.join("、")
            }
        );
    }
    Ok(())
}

fn label(kind: OpKind) -> &'static str {
    match kind {
        OpKind::Move => "移动",
        OpKind::Copy => "复制",
        OpKind::Extract => "解压",
        OpKind::Compress => "压缩",
        OpKind::Shortcut => "快捷方式",
        OpKind::Rename => "改名",
        OpKind::Trash => "回收",
    }
}

fn print_plan(plan: &Plan) {
    for w in &plan.warnings {
        println!("⚠ {w}");
    }
    let mut current_rule: Option<&str> = None;
    for op in &plan.ops {
        if current_rule != Some(op.rule.as_str()) {
            println!("\n▸ {}", op.rule);
            current_rule = Some(&op.rule);
        }
        let target = match (&op.kind, &op.dst) {
            (OpKind::Rename, Some(d)) => d
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            (_, Some(d)) => display(d),
            (_, None) => "回收站".to_string(),
        };
        let src = if op.kind == OpKind::Rename {
            op.src
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            display(&op.src)
        };
        let note = op
            .note
            .as_deref()
            .map(|n| format!("  ({n})"))
            .unwrap_or_default();
        println!("  {}  {src} → {target}{note}", label(op.kind));
    }
    // 需要用户留意的跳过逐条列出，其余按原因汇总
    let notable: Vec<_> = plan
        .skipped
        .iter()
        .filter(|s| {
            matches!(
                s.kind,
                SkipKind::Conflict | SkipKind::Invalid | SkipKind::InUse
            )
        })
        .collect();
    if !notable.is_empty() {
        println!("\n需要留意（{} 个）：", notable.len());
        for s in notable {
            println!("  - {}  ({})", display(&s.path), s.reason);
        }
    }
    let summary: Vec<String> = plan
        .skip_summary()
        .into_iter()
        .map(|(k, n)| format!("{} {n}", k.label()))
        .collect();
    if !summary.is_empty() {
        println!("\n保持原位：{}", summary.join("，"));
    }
    println!(
        "\n扫描 {} 个文件，命中 {} 个，已在目标位置 {} 个，计划 {} 项操作。",
        plan.scanned,
        plan.matched,
        plan.in_place,
        plan.ops.len()
    );
    if plan.is_empty() {
        println!("没有需要处理的文件。");
    }
}

fn history(data_dir: &Path, limit: usize) -> Result<()> {
    let journal = Journal::open_in(data_dir)?;
    let batches = journal.batches(limit)?;
    if batches.is_empty() {
        println!("还没有执行记录。");
        return Ok(());
    }
    for b in batches {
        println!(
            "#{:<4} {}  {:<4} {:>4} 项  {}",
            b.id,
            b.started_at,
            b.source,
            b.op_count,
            status_label(&b.status)
        );
    }
    Ok(())
}

fn status_label(s: &str) -> &str {
    match s {
        status::RUNNING => "执行中/中断",
        status::DONE => "完成",
        status::FAILED => "失败",
        status::PARTIAL => "部分完成",
        status::UNDONE => "已撤销",
        status::PARTIALLY_UNDONE => "部分撤销",
        other => other,
    }
}

fn undo(data_dir: &Path, batch: Option<i64>, yes: bool) -> Result<()> {
    let journal = Journal::open_in(data_dir)?;
    let id = match batch {
        Some(id) => id,
        None => match journal.last_undoable()? {
            Some(id) => id,
            None => {
                println!("没有可以撤销的执行记录。");
                return Ok(());
            }
        },
    };
    let Some(info) = journal.batch(id)? else {
        bail!("找不到批次 #{id}");
    };
    if !yes
        && !confirm(&format!(
            "撤销批次 #{id}（{}，{} 项操作）？",
            info.started_at, info.op_count
        ))?
    {
        println!("已取消。");
        return Ok(());
    }
    let report = undo_batch(&journal, id)?;
    println!("✓ 已恢复 {} 个文件。", report.restored);
    if !report.in_trash.is_empty() {
        println!("以下文件已在系统回收站中，请手动还原：");
        for p in &report.in_trash {
            println!("  - {}", display(p));
        }
    }
    if !report.failed.is_empty() {
        println!("以下文件无法撤销：");
        for (p, e) in &report.failed {
            println!("  ✗ {}  ({e})", display(p));
        }
        bail!("有 {} 项无法撤销", report.failed.len());
    }
    Ok(())
}

fn inspect(path: &Path, all: bool, rules_path: &Path) -> Result<()> {
    use foldo_core::fields::{self, Value};
    let mut file = foldo_core::meta::FileCtx::from_path(path)
        .with_context(|| format!("无法读取 {}", display(path)))?;
    if rules_path.exists() {
        file.set_categories(&load(rules_path)?.categories);
    }
    println!("{}", display(path));
    let mut group = "";
    for (def, value) in fields::inspect(&file) {
        if value.is_none() && !all {
            continue;
        }
        if def.group != group {
            group = def.group;
            println!("\n{group}");
        }
        let shown = match &value {
            Some(Value::Size(b)) => format!("{b} 字节（{}）", human_size(*b)),
            Some(v) => format!(
                "{}{}",
                v.display(),
                def.unit
                    .filter(|u| !u.contains('/'))
                    .map(|u| format!(" {u}"))
                    .unwrap_or_default()
            ),
            None => "—".into(),
        };
        println!("  {:<20} {:<10} {shown}", def.key, def.label);
    }
    Ok(())
}

fn human_size(n: u64) -> String {
    let mut v = n as f64;
    for unit in ["B", "KB", "MB", "GB", "TB"] {
        if v < 1000.0 || unit == "TB" {
            return if unit == "B" {
                format!("{n} B")
            } else {
                format!("{v:.1} {unit}")
            };
        }
        v /= 1000.0;
    }
    unreachable!()
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(line.trim().to_lowercase().as_str(), "y" | "yes"))
}
