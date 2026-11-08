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

/// 命令行的语言：环境变量 FOLDO_LANG 优先，其次是系统的 LC_ALL / LC_MESSAGES / LANG；都没有时用简体中文。
fn init_locale() {
    let lang = ["FOLDO_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .filter_map(|k| std::env::var(k).ok())
        .find(|v| !v.is_empty() && v != "C" && v != "POSIX");
    if let Some(l) = lang {
        foldo_core::i18n::set_locale(&l);
    }
}

fn main() {
    init_locale();
    if let Err(e) = run() {
        eprintln!("{}", foldo_core::t!("错误: {e}", e = format!("{e:#}")));
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
                    "{}",
                    foldo_core::t!(
                        "\n⚠ 本次操作数量较多（{a0} 项），请仔细核对。",
                        a0 = plan.ops.len()
                    )
                );
            }
            if !yes
                && !confirm(&foldo_core::t!(
                    "\n执行以上 {a0} 项操作？",
                    a0 = plan.ops.len()
                ))?
            {
                println!("{}", foldo_core::t!("已取消。"));
                return Ok(());
            }
            let journal = Journal::open_in(&data_dir)?;
            let report = execute(&plan, &journal, "cli")?;
            println!(
                "{}",
                foldo_core::t!(
                    "\n✓ 批次 #{a0}：完成 {a1} 项，跳过 {a2} 项，失败 {a3} 项。",
                    a0 = report.batch_id,
                    a1 = report.done,
                    a2 = report.skipped.len(),
                    a3 = report.failed.len()
                )
            );
            for (op, err) in &report.skipped {
                println!("  ↷ {}  ({err})", display(&op.src));
            }
            for (op, err) in &report.failed {
                println!("  ✗ {}  ({err})", display(&op.src));
            }
            if report.done > 0 {
                println!(
                    "{}",
                    foldo_core::t!("如需撤销：foldo undo {a0}", a0 = report.batch_id)
                );
            }
            if !report.failed.is_empty() {
                bail!(
                    "{}",
                    foldo_core::t!("有 {a0} 项操作失败", a0 = report.failed.len())
                );
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
            "{}",
            foldo_core::t!(
                "规则文件不存在：{a0}\n可以先运行 `foldo init` 生成示例规则，或用 --rules 指定路径。",
                a0 = display(path)
            )
        );
    }
    Ok(RuleSet::load(path)?)
}

fn init(path: &Path, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "{}",
            foldo_core::t!(
                "规则文件已存在：{a0}（使用 --force 覆盖）",
                a0 = display(path)
            )
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| foldo_core::t!("无法创建目录 {a0}", a0 = display(parent)))?;
    }
    std::fs::write(path, EXAMPLE_RULES)
        .with_context(|| foldo_core::t!("无法写入 {a0}", a0 = display(path)))?;
    println!(
        "{}",
        foldo_core::t!("✓ 已生成示例规则：{a0}", a0 = display(path))
    );
    println!("{}", foldo_core::t!("编辑后运行 `foldo plan` 预览效果。"));
    Ok(())
}

fn check(path: &Path) -> Result<()> {
    let rules = load(path)?;
    let enabled = rules.rules.iter().filter(|r| r.enabled).count();
    println!(
        "{}",
        foldo_core::t!(
            "✓ 规则文件有效：{a0}，共 {a1} 条规则，{a2} 条启用",
            a0 = display(path),
            a1 = rules.rules.len(),
            a2 = enabled
        )
    );
    for r in &rules.rules {
        println!("  {} {}", if r.enabled { "●" } else { "○" }, r.name);
    }
    println!("{}", foldo_core::t!("整理文件夹："));
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
                foldo_core::t!("（含子文件夹）")
            } else {
                String::new()
            },
            if names.is_empty() {
                foldo_core::t!("（没有使用规则）")
            } else {
                names.join(&foldo_core::t!("、"))
            }
        );
    }
    Ok(())
}

fn label(kind: OpKind) -> String {
    foldo_core::i18n::tr(match kind {
        OpKind::Move => "移动",
        OpKind::Copy => "复制",
        OpKind::Extract => "解压",
        OpKind::Compress => "压缩",
        OpKind::Shortcut => "快捷方式",
        OpKind::Rename => "改名",
        OpKind::Trash => "回收",
    })
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
            (_, None) => foldo_core::t!("回收站"),
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
        println!(
            "{}",
            foldo_core::t!("\n需要留意（{a0} 个）：", a0 = notable.len())
        );
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
        println!(
            "{}",
            foldo_core::t!("\n保持原位：{a0}", a0 = summary.join("，"))
        );
    }
    println!(
        "{}",
        foldo_core::t!(
            "\n扫描 {a0} 个文件，命中 {a1} 个，已在目标位置 {a2} 个，计划 {a3} 项操作。",
            a0 = plan.scanned,
            a1 = plan.matched,
            a2 = plan.in_place,
            a3 = plan.ops.len()
        )
    );
    if plan.is_empty() {
        println!("{}", foldo_core::t!("没有需要处理的文件。"));
    }
}

fn history(data_dir: &Path, limit: usize) -> Result<()> {
    let journal = Journal::open_in(data_dir)?;
    let batches = journal.batches(limit)?;
    if batches.is_empty() {
        println!("{}", foldo_core::t!("还没有执行记录。"));
        return Ok(());
    }
    for b in batches {
        println!(
            "#{:<4} {}  {:<4} {:>6}  {}",
            b.id,
            b.started_at,
            b.source,
            foldo_core::t!("{n} 项", n = b.op_count),
            status_label(&b.status)
        );
    }
    Ok(())
}

fn status_label(s: &str) -> String {
    foldo_core::i18n::tr(match s {
        status::RUNNING => "执行中/中断",
        status::DONE => "完成",
        status::FAILED => "失败",
        status::PARTIAL => "部分完成",
        status::UNDONE => "已撤销",
        status::PARTIALLY_UNDONE => "部分撤销",
        other => other,
    })
}

fn undo(data_dir: &Path, batch: Option<i64>, yes: bool) -> Result<()> {
    let journal = Journal::open_in(data_dir)?;
    let id = match batch {
        Some(id) => id,
        None => match journal.last_undoable()? {
            Some(id) => id,
            None => {
                println!("{}", foldo_core::t!("没有可以撤销的执行记录。"));
                return Ok(());
            }
        },
    };
    let Some(info) = journal.batch(id)? else {
        bail!("{}", foldo_core::t!("找不到批次 #{id}", id = id));
    };
    if !yes
        && !confirm(&foldo_core::t!(
            "撤销批次 #{id}（{a0}，{a1} 项操作）？",
            id = id,
            a0 = info.started_at,
            a1 = info.op_count
        ))?
    {
        println!("{}", foldo_core::t!("已取消。"));
        return Ok(());
    }
    let report = undo_batch(&journal, id)?;
    println!(
        "{}",
        foldo_core::t!("✓ 已恢复 {a0} 个文件。", a0 = report.restored)
    );
    if !report.in_trash.is_empty() {
        println!(
            "{}",
            foldo_core::t!("以下文件已在系统回收站中，请手动还原：")
        );
        for p in &report.in_trash {
            println!("  - {}", display(p));
        }
    }
    if !report.failed.is_empty() {
        println!("{}", foldo_core::t!("以下文件无法撤销："));
        for (p, e) in &report.failed {
            println!("  ✗ {}  ({e})", display(p));
        }
        bail!(
            "{}",
            foldo_core::t!("有 {a0} 项无法撤销", a0 = report.failed.len())
        );
    }
    Ok(())
}

fn inspect(path: &Path, all: bool, rules_path: &Path) -> Result<()> {
    use foldo_core::fields::{self, Value};
    let mut file = foldo_core::meta::FileCtx::from_path(path)
        .with_context(|| foldo_core::t!("无法读取 {a0}", a0 = display(path)))?;
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
            println!("\n{}", foldo_core::i18n::tr(group));
        }
        let shown = match &value {
            Some(Value::Size(b)) => foldo_core::t!("{b} 字节（{a0}）", b = b, a0 = human_size(*b)),
            Some(v) => format!(
                "{}{}",
                foldo_core::i18n::tr(&v.display()),
                def.unit
                    .filter(|u| !u.contains('/'))
                    .map(|u| format!(" {u}"))
                    .unwrap_or_default()
            ),
            None => "—".into(),
        };
        println!(
            "  {:<20} {:<10} {shown}",
            def.key,
            foldo_core::i18n::tr(def.label)
        );
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
