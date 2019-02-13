//! `athena-bench` — Athena 内核基准 CLI（JSON / Markdown 报告由 Rust 直接生成）。

use std::{path::PathBuf, process::ExitCode};

use athena_benchmark::{
    fixture::{BenchGroup, RunConfig},
    groups::default_suite,
    report::ReportTier,
    run_suite,
};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "athena-bench", about = "Athena 内核基准（确定性 fixture + 内置报告）")]
struct Args {
    /// 逗号分隔分组：numeric,bigint,path,ir,rewriter,engine,domains,jit,infra（默认全部）
    #[arg(long, value_delimiter = ',')]
    groups: Vec<String>,

    /// 每个 fixture 的预热次数
    #[arg(long, default_value_t = 3)]
    warmup: usize,

    /// 每个 fixture 的计时采样次数
    #[arg(long, default_value_t = 25)]
    samples: usize,

    /// 非 bigint fixture 的默认分层：kernel / arena / end_to_end
    #[arg(long, default_value = "end_to_end")]
    tier: String,

    /// 输出格式：text / json / markdown（可与 `--write` 联用）
    #[arg(long, default_value = "text")]
    format: String,

    /// 写入报告文件（`.json` → JSON，其它 → Markdown）；省略则打印到 stdout
    #[arg(long)]
    write: Option<PathBuf>,

    /// 兼容旧开关：等价于 `--format json`
    #[arg(long)]
    json: bool,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let mut groups = Vec::new();
    for g in &args.groups {
        match BenchGroup::parse(g) {
            Some(group) => groups.push(group),
            None => {
                eprintln!("unknown group `{g}` (expected numeric|bigint|path|ir|rewriter|engine|domains|jit|infra)");
                return ExitCode::from(2);
            }
        }
    }

    let report_tier = match args.tier.as_str() {
        "kernel" => ReportTier::Kernel,
        "arena" => ReportTier::Arena,
        "end_to_end" | "e2e" => ReportTier::EndToEnd,
        other => {
            eprintln!("unknown tier `{other}` (expected kernel|arena|end_to_end)");
            return ExitCode::from(2);
        }
    };

    let format = if args.json { "json" } else { args.format.as_str() };

    let config = RunConfig { groups, warmup: args.warmup, samples: args.samples, report_tier };

    let suite = default_suite();
    match run_suite(&suite, &config) {
        Ok(report) => {
            if let Some(path) = &args.write {
                if let Err(e) = report.write_to(path) {
                    eprintln!("write failed: {e}");
                    return ExitCode::FAILURE;
                }
                eprintln!("wrote {}", path.display());
            }

            match format {
                "json" => match report.to_json_pretty() {
                    Ok(s) => {
                        if args.write.is_none() {
                            println!("{s}");
                        }
                    }
                    Err(e) => {
                        eprintln!("json encode failed: {e}");
                        return ExitCode::FAILURE;
                    }
                },
                "markdown" | "md" => {
                    if args.write.is_none() {
                        print!("{}", report.to_markdown());
                    }
                }
                "text" => {
                    if args.write.is_none() {
                        println!(
                            "athena-bench  commit={}  rustc={}  target={}  threads={}  jit={}",
                            report.env.commit.as_deref().unwrap_or("?"),
                            report.env.rustc.as_deref().unwrap_or("?"),
                            report.env.target_triple,
                            report.env.threads,
                            report.env.jit_enabled
                        );
                        for f in &report.fixtures {
                            if f.skipped {
                                println!("  SKIP  {:<40}  ({})", f.id, f.fallback_reason.as_deref().unwrap_or("skipped"));
                            }
                            else {
                                println!(
                                    "  OK    {:<40}  layer={}  ctx={}  gc={}  p50={}  {}",
                                    f.id,
                                    f.layer.map(|l| l.as_str()).unwrap_or("-"),
                                    f.context_policy.map(|c| c.as_str()).unwrap_or("-"),
                                    f.gc_mode.as_deref().unwrap_or("?"),
                                    f.p50_ns.map(|n| n.to_string()).unwrap_or_else(|| "-".into()),
                                    f.validation.notes
                                );
                            }
                        }
                    }
                }
                other => {
                    eprintln!("unknown format `{other}` (expected text|json|markdown)");
                    return ExitCode::from(2);
                }
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}
