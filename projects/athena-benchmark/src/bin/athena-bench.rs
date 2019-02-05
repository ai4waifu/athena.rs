//! `athena-bench` — Athena 内核基准 CLI。

use std::process::ExitCode;

use athena_benchmark::{
    fixture::{BenchGroup, RunConfig},
    groups::default_suite,
    report::ReportTier,
    run_suite,
};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "athena-bench", about = "Athena 内核基准（确定性 fixture）")]
struct Args {
    /// 逗号分隔分组：numeric,ir,rewriter,engine,domains,jit,infra（默认全部）
    #[arg(long, value_delimiter = ',')]
    groups: Vec<String>,

    /// 每个 fixture 的预热次数
    #[arg(long, default_value_t = 3)]
    warmup: usize,

    /// 每个 fixture 的计时采样次数
    #[arg(long, default_value_t = 25)]
    samples: usize,

    /// 报告分层：kernel / arena / end_to_end（默认 end_to_end）
    #[arg(long, default_value = "end_to_end")]
    tier: String,

    /// 输出机器可读 JSON 报告
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
                eprintln!("unknown group `{g}` (expected numeric|bigint|ir|rewriter|engine|domains|jit)");
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

    let config = RunConfig { groups, warmup: args.warmup, samples: args.samples, report_tier };

    let suite = default_suite();
    match run_suite(&suite, &config) {
        Ok(report) => {
            if args.json {
                match report.to_json_pretty() {
                    Ok(s) => println!("{s}"),
                    Err(e) => {
                        eprintln!("json encode failed: {e}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            else {
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
                        println!("  SKIP  {:<32}  ({})", f.id, f.fallback_reason.as_deref().unwrap_or("skipped"));
                    }
                    else {
                        println!(
                            "  OK    {:<32}  tier={}  gc={}  p50={}ns  arena={}  scratch={}  {}",
                            f.id,
                            f.report_tier.map(|t| t.as_str()).unwrap_or("?"),
                            f.gc_mode.as_deref().unwrap_or("?"),
                            f.p50_ns.unwrap_or(0),
                            f.peak_arena_bytes.unwrap_or(0),
                            f.peak_scratch_bytes.unwrap_or(0),
                            f.validation.notes
                        );
                    }
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
