//! 机器可读基准报告。

use serde::Serialize;

use crate::{env::BenchEnv, validate::ValidationSummary};

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
}
