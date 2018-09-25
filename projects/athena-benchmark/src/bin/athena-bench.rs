//! `athena-bench` — Athena 内核基准 CLI。

use std::process::ExitCode;

use athena_benchmark::{
    fixture::{BenchGroup, RunConfig},
    groups::default_suite,
    run_suite,
};
use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "athena-bench", about = "Athena kernel benchmarks (deterministic fixtures)")]
struct Args {
    /// Comma-separated groups: numeric,bigint,ir,rewriter,engine,domains,jit (default: all)
    #[arg(long, value_delimiter = ',')]
    groups: Vec<String>,

    /// Warmup iterations per fixture
    #[arg(long, default_value_t = 3)]
    warmup: usize,

    /// Timed samples per fixture
    #[arg(long, default_value_t = 25)]
    samples: usize,

    /// Emit machine-readable JSON report
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

    let config = RunConfig { groups, warmup: args.warmup, samples: args.samples };

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
                            "  OK    {:<32}  p50={}ns  p95={}ns  {}",
                            f.id,
                            f.p50_ns.unwrap_or(0),
                            f.p95_ns.unwrap_or(0),
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
