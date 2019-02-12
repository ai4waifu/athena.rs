//! 机器可读基准报告（JSON / Markdown，纯 Rust 生成）。

use std::{fmt::Write as _, fs, path::Path};

use serde::Serialize;

use crate::{
    bigint::{BenchLayer, ContextPolicy},
    env::BenchEnv,
    validate::ValidationSummary,
};

/// Living `15`/`12` 三类报告分层（禁止互相冒充）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportTier {
    /// Limb / machine kernel：建议 `GcMode::Disabled`。
    Kernel,
    /// Arena / 分配：建议 `GcMode::Deferred`。
    Arena,
    /// 端到端 CAS：建议 `GcMode::Auto`。
    EndToEnd,
}

impl ReportTier {
    /// 稳定短名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Arena => "arena",
            Self::EndToEnd => "end_to_end",
        }
    }

    /// 对应建议的 `GcMode` 名（报告字段，非强制切换）。
    pub fn suggested_gc_mode(self) -> &'static str {
        match self {
            Self::Kernel => "disabled",
            Self::Arena => "deferred",
            Self::EndToEnd => "auto",
        }
    }
}

/// 完整报告（一次 `athena-bench` 运行）。
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    /// 环境快照。
    pub env: BenchEnv,
    /// 各 fixture 结果。
    pub fixtures: Vec<FixtureReport>,
}

/// 单个 fixture 的报告行。
#[derive(Debug, Clone, Serialize)]
pub struct FixtureReport {
    /// Fixture 稳定 id。
    pub id: String,
    /// 分组名（numeric / ir / …）。
    pub group: String,
    /// 数据规模描述。
    pub scale: String,
    /// 数值域或 IR 域标签。
    pub domain: String,
    /// 预热次数。
    pub warmup: usize,
    /// 采样次数。
    pub samples: usize,
    /// 是否跳过（例如未启用 jit）。
    pub skipped: bool,
    /// 中位耗时（纳秒）；跳过时为 `null`。
    pub p50_ns: Option<u64>,
    /// 95 分位耗时（纳秒）；跳过时为 `null`。
    pub p95_ns: Option<u64>,
    /// 分配字节（尽力而为，常为 `null`）。
    pub alloc_bytes: Option<u64>,
    /// 峰值 RSS（尽力而为，常为 `null`）。
    pub peak_rss_bytes: Option<u64>,
    /// 报告分层（kernel / arena / end_to_end）。
    pub report_tier: Option<ReportTier>,
    /// 测量层（kernel / numeric / e2e / peer）。
    pub layer: Option<BenchLayer>,
    /// Context 策略（reused / per_call）。
    pub context_policy: Option<ContextPolicy>,
    /// 实现名（athena / num / ibig / malachite）。
    pub implementation: Option<String>,
    /// 操作名（add / mul / …）。
    pub operation: Option<String>,
    /// 位宽。
    pub bits: Option<u32>,
    /// 本次采样有效的 `GcMode` 名。
    pub gc_mode: Option<String>,
    /// 峰值 arena resident 字节。
    pub peak_arena_bytes: Option<u64>,
    /// 峰值 scratch 字节。
    pub peak_scratch_bytes: Option<u64>,
    /// 累计 GC 时间（纳秒）。
    pub gc_time_ns: Option<u64>,
    /// 校验摘要。
    pub validation: ValidationSummary,
    /// 跳过或回退原因。
    pub fallback_reason: Option<String>,
}

impl Report {
    /// 序列化为 JSON 字符串。
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 生成 Markdown 汇总（bigint 矩阵透视表 + 其它 fixture 列表）。
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# athena-bench report\n\n- commit: `{}`\n- rustc: `{}`\n- target: `{}`\n- threads: {}\n- jit: {}\n- fixtures: {}\n",
            self.env.commit.as_deref().unwrap_or("?"),
            self.env.rustc.as_deref().unwrap_or("?"),
            self.env.target_triple,
            self.env.threads,
            self.env.jit_enabled,
            self.fixtures.len()
        );

        let bigint: Vec<&FixtureReport> = self.fixtures.iter().filter(|f| f.group == "bigint").collect();
        let other: Vec<&FixtureReport> = self.fixtures.iter().filter(|f| f.group != "bigint").collect();

        if !bigint.is_empty() {
            out.push_str("## bigint matrix\n\n");
            out.push_str(&render_bigint_markdown(&bigint));
        }

        if !other.is_empty() {
            out.push_str("## other fixtures\n\n");
            out.push_str("| id | tier | gc | p50 | notes |\n|---|---|---|---:|---|\n");
            for f in other {
                let p50 = f.p50_ns.map(format_ns).unwrap_or_else(|| "—".into());
                let notes = if f.skipped {
                    f.fallback_reason.as_deref().unwrap_or("skipped")
                }
                else {
                    f.validation.notes.as_str()
                };
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} | {} | {} |",
                    f.id,
                    f.report_tier.map(|t| t.as_str()).unwrap_or("?"),
                    f.gc_mode.as_deref().unwrap_or("?"),
                    p50,
                    notes
                );
            }
            out.push('\n');
        }

        out
    }

    /// 按扩展名写入：`.json` → JSON，`.md` / 其它 → Markdown。
    pub fn write_to(&self, path: impl AsRef<Path>) -> Result<(), String> {
        let path = path.as_ref();
        let body = match path.extension().and_then(|e| e.to_str()) {
            Some("json") => self.to_json_pretty().map_err(|e| e.to_string())?,
            _ => self.to_markdown(),
        };
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
        }
        fs::write(path, body).map_err(|e| e.to_string())
    }
}

fn render_bigint_markdown(rows: &[&FixtureReport]) -> String {
    let mut out = String::new();
    let mut ops: Vec<String> = Vec::new();
    for r in rows {
        if let Some(op) = &r.operation {
            if !ops.iter().any(|x| x == op) {
                ops.push(op.clone());
            }
        }
    }

    let layers = [
        (BenchLayer::Kernel, ContextPolicy::Reused),
        (BenchLayer::Numeric, ContextPolicy::Reused),
        (BenchLayer::E2e, ContextPolicy::PerCall),
    ];

    for op in &ops {
        for &(layer, policy) in &layers {
            let section: Vec<&&FixtureReport> = rows
                .iter()
                .filter(|r| {
                    r.operation.as_deref() == Some(op.as_str())
                        && (r.layer == Some(layer)
                            || (r.layer == Some(BenchLayer::Peer) && layer == BenchLayer::Numeric))
                })
                .collect();
            // peers 挂在 numeric 算法对照表；kernel/e2e 只展示 athena 同行位宽
            let athena_rows: Vec<&&&FixtureReport> = section
                .iter()
                .filter(|r| r.implementation.as_deref() == Some("athena") && r.layer == Some(layer))
                .collect();
            if athena_rows.is_empty() {
                continue;
            }

            let _ = writeln!(
                out,
                "### `{op}` · layer=`{}` · context=`{}` · gc=`{}`\n",
                layer.as_str(),
                policy.as_str(),
                layer.suggested_gc_mode()
            );

            let mut bits: Vec<u32> = athena_rows.iter().filter_map(|r| r.bits).collect();
            bits.sort_unstable();
            bits.dedup();

            if layer == BenchLayer::Numeric {
                out.push_str("| bits | athena | num | ibig | malachite |\n|-----:|-------:|----:|-----:|----------:|\n");
                for bit in bits {
                    let ath = find_p50(rows, op, bit, "athena", Some(BenchLayer::Numeric));
                    let num = find_p50(rows, op, bit, "num", Some(BenchLayer::Peer));
                    let ibig = find_p50(rows, op, bit, "ibig", Some(BenchLayer::Peer));
                    let mal = find_p50(rows, op, bit, "malachite", Some(BenchLayer::Peer));
                    let _ = writeln!(
                        out,
                        "| {bit} | {} | {} | {} | {} |",
                        fmt_cell(ath),
                        fmt_cell(num),
                        fmt_cell(ibig),
                        fmt_cell(mal)
                    );
                }
                out.push('\n');
                out.push_str("相对 athena（athena = 1×，值 = peer / athena，越小越快）\n\n");
                out.push_str("| lib | ");
                for bit in &bits {
                    let _ = write!(out, "{bit} | ");
                }
                out.push('\n|---|');
                for _ in &bits {
                    out.push("---:|");
                }
                out.push('\n');
                for lib in ["athena", "num", "ibig", "malachite"] {
                    let _ = write!(out, "| {lib} | ");
                    for bit in &bits {
                        let ath = find_p50(rows, op, *bit, "athena", Some(BenchLayer::Numeric));
                        let peer = if lib == "athena" {
                            ath
                        }
                        else {
                            find_p50(rows, op, *bit, lib, Some(BenchLayer::Peer))
                        };
                        let _ = write!(out, "{} | ", fmt_ratio(ath, peer));
                    }
                    out.push('\n');
                }
                out.push('\n');
            }
            else {
                out.push_str("| bits | athena | gc | context |\n|-----:|-------:|---|---|\n");
                for bit in bits {
                    let ath = find_p50(rows, op, bit, "athena", Some(layer));
                    let _ = writeln!(
                        out,
                        "| {bit} | {} | {} | {} |",
                        fmt_cell(ath),
                        layer.suggested_gc_mode(),
                        policy.as_str()
                    );
                }
                out.push('\n');
            }
        }
    }

    out
}

fn find_p50(rows: &[&FixtureReport], op: &str, bits: u32, impl_name: &str, layer: Option<BenchLayer>) -> Option<u64> {
    rows.iter().find_map(|r| {
        if r.skipped {
            return None;
        }
        if r.operation.as_deref() != Some(op) {
            return None;
        }
        if r.bits != Some(bits) {
            return None;
        }
        if r.implementation.as_deref() != Some(impl_name) {
            return None;
        }
        if layer.is_some() && r.layer != layer {
            return None;
        }
        r.p50_ns
    })
}

fn fmt_cell(ns: Option<u64>) -> String {
    ns.map(format_ns).unwrap_or_else(|| "—".into())
}

fn fmt_ratio(base: Option<u64>, peer: Option<u64>) -> String {
    match (base, peer) {
        (Some(b), Some(p)) if b > 0 => format!("{:.2}×", p as f64 / b as f64),
        _ => "—".into(),
    }
}

fn format_ns(ns: u64) -> String {
    if ns >= 1_000_000 {
        format!("{:.2} ms", ns as f64 / 1_000_000.0)
    }
    else if ns >= 1_000 {
        format!("{:.2} µs", ns as f64 / 1_000.0)
    }
    else {
        format!("{ns} ns")
    }
}
