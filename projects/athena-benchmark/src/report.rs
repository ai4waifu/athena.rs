//! 机器可读合同 / 资源报告（JSON / Markdown，纯 Rust 生成）。
//!
//! **不含 ns/op。** 性能数字只来自 Criterion；本报告记录校验、layer/context/gc 与资源采样。

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

/// 单个 fixture 的合同 / 资源报告行。
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
    /// 冒烟执行次数（不计时）。
    pub smoke_iters: usize,
    /// 是否跳过（例如未启用 jit）。
    pub skipped: bool,
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
    /// 提醒：性能计时不在本报告。
    pub timing_note: Option<String>,
}

impl Report {
    /// 序列化为 JSON 字符串。
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// 生成 Markdown 汇总（校验矩阵 + 资源字段；**无 ns/op 表**）。
    pub fn to_markdown(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "# athena-bench contract report\n\n- commit: `{}`\n- rustc: `{}`\n- target: `{}`\n- threads: {}\n- jit: {}\n- fixtures: {}\n\n> Performance ns/op is **Criterion-only** (`cargo bench`). This report is validation + resource sampling.\n",
            self.env.commit.as_deref().unwrap_or("?"),
            self.env.rustc.as_deref().unwrap_or("?"),
            self.env.target_triple,
            self.env.threads,
            self.env.jit_enabled,
            self.fixtures.len()
        );

        let path_rows: Vec<&FixtureReport> = self.fixtures.iter().filter(|f| f.group == "path").collect();
        let bigint: Vec<&FixtureReport> = self.fixtures.iter().filter(|f| f.group == "bigint").collect();
        let other: Vec<&FixtureReport> =
            self.fixtures.iter().filter(|f| f.group != "bigint" && f.group != "path").collect();

        if !path_rows.is_empty() {
            out.push_str("## path segments（Living 18）\n\n");
            out.push_str("| id | layer | ctx | gc | ok | arena | scratch | notes |\n|---|---|---|---|---|---:|---:|---|\n");
            for f in path_rows {
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} | {} | {} | {} | {} | {} |",
                    f.id,
                    f.layer.map(|l| l.as_str()).unwrap_or("-"),
                    f.context_policy.map(|c| c.as_str()).unwrap_or("-"),
                    f.gc_mode.as_deref().unwrap_or("?"),
                    status_cell(f),
                    fmt_opt_u64(f.peak_arena_bytes),
                    fmt_opt_u64(f.peak_scratch_bytes),
                    notes_cell(f)
                );
            }
            out.push('\n');
        }

        if !bigint.is_empty() {
            out.push_str("## bigint matrix（validation）\n\n");
            out.push_str(&render_bigint_validation(&bigint));
        }

        if !other.is_empty() {
            out.push_str("## other fixtures\n\n");
            out.push_str("| id | layer | ctx | gc | ok | notes |\n|---|---|---|---|---|---|\n");
            for f in other {
                let _ = writeln!(
                    out,
                    "| `{}` | {} | {} | {} | {} | {} |",
                    f.id,
                    f.layer.map(|l| l.as_str()).unwrap_or("-"),
                    f.context_policy.map(|c| c.as_str()).unwrap_or("-"),
                    f.gc_mode.as_deref().unwrap_or("?"),
                    status_cell(f),
                    notes_cell(f)
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

fn render_bigint_validation(rows: &[&FixtureReport]) -> String {
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
                out.push_str("| bits | athena | num | ibig | malachite |\n|-----:|:------:|:---:|:----:|:---------:|\n");
                for &bit in &bits {
                    let _ = writeln!(
                        out,
                        "| {bit} | {} | {} | {} | {} |",
                        ok_mark(find_row(rows, op, bit, "athena", Some(BenchLayer::Numeric))),
                        ok_mark(find_row(rows, op, bit, "num", Some(BenchLayer::Peer))),
                        ok_mark(find_row(rows, op, bit, "ibig", Some(BenchLayer::Peer))),
                        ok_mark(find_row(rows, op, bit, "malachite", Some(BenchLayer::Peer)))
                    );
                }
                out.push('\n');
            }
            else {
                out.push_str("| bits | athena | gc | context |\n|-----:|:------:|---|---|\n");
                for bit in bits {
                    let _ = writeln!(
                        out,
                        "| {bit} | {} | {} | {} |",
                        ok_mark(find_row(rows, op, bit, "athena", Some(layer))),
                        layer.suggested_gc_mode(),
                        policy.as_str()
                    );
                }
                out.push('\n');
            }
        }
    }

    out.push_str("Timing: use `cargo bench -p athena-benchmark --features compare-bigint --bench compare_bigint`.\n\n");
    out
}

fn find_row<'a>(
    rows: &'a [&'a FixtureReport],
    op: &str,
    bits: u32,
    impl_name: &str,
    layer: Option<BenchLayer>,
) -> Option<&'a FixtureReport> {
    rows.iter().copied().find(|r| {
        if r.skipped {
            return false;
        }
        if r.operation.as_deref() != Some(op) {
            return false;
        }
        if r.bits != Some(bits) {
            return false;
        }
        if r.implementation.as_deref() != Some(impl_name) {
            return false;
        }
        if layer.is_some() && r.layer != layer {
            return false;
        }
        true
    })
}

fn ok_mark(row: Option<&FixtureReport>) -> &'static str {
    match row {
        Some(r) if r.validation.ok && !r.skipped => "ok",
        Some(_) => "fail",
        None => "—",
    }
}

fn status_cell(f: &FixtureReport) -> &'static str {
    if f.skipped {
        "skip"
    }
    else if f.validation.ok {
        "ok"
    }
    else {
        "fail"
    }
}

fn notes_cell(f: &FixtureReport) -> &str {
    if f.skipped {
        f.fallback_reason.as_deref().unwrap_or("skipped")
    }
    else {
        f.validation.notes.as_str()
    }
}

fn fmt_opt_u64(v: Option<u64>) -> String {
    v.map(|n| n.to_string()).unwrap_or_else(|| "—".into())
}
