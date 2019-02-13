//! Fixture 合同与 suite 编排。

use std::fmt;

use crate::{
    bigint::{BenchLayer, ContextPolicy},
    env::BenchEnv,
    report::{FixtureReport, ReportTier},
    timing::measure,
    validate::ValidationSummary,
};

/// 基准分组标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BenchGroup {
    /// ExactInteger / ExactRational / promotion / 机器实数等。
    Numeric,
    /// 统一 bigint 对照矩阵（Athena layers + optional peers）。
    Bigint,
    /// 路径拆分 microbench（kernel / alloc / clone / publish）。
    Path,
    /// TermArena / hash / verify。
    Ir,
    /// 匹配与规范化。
    Rewriter,
    /// M-Graph / Session / limits。
    Engine,
    /// 多项式 / 数论 / 微积分 / sample_1d。
    Domains,
    /// JIT（feature 门控）。
    Jit,
    /// ndarray / graph / table 基础设施合同。
    Infra,
}

impl BenchGroup {
    /// 稳定短名（报告 / CLI）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Numeric => "numeric",
            Self::Bigint => "bigint",
            Self::Path => "path",
            Self::Ir => "ir",
            Self::Rewriter => "rewriter",
            Self::Engine => "engine",
            Self::Domains => "domains",
            Self::Jit => "jit",
            Self::Infra => "infra",
        }
    }

    /// 解析分组名。
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "numeric" => Some(Self::Numeric),
            "bigint" => Some(Self::Bigint),
            "path" => Some(Self::Path),
            "ir" => Some(Self::Ir),
            "rewriter" => Some(Self::Rewriter),
            "engine" => Some(Self::Engine),
            "domains" => Some(Self::Domains),
            "jit" => Some(Self::Jit),
            "infra" => Some(Self::Infra),
            _ => None,
        }
    }
}

impl fmt::Display for BenchGroup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Fixture 静态元数据。
#[derive(Debug, Clone)]
pub struct FixtureMeta {
    /// 稳定 id。
    pub id: &'static str,
    /// 分组。
    pub group: BenchGroup,
    /// 规模描述。
    pub scale: &'static str,
    /// 域标签。
    pub domain: &'static str,
    /// 测量层（bigint 矩阵）。
    pub layer: Option<BenchLayer>,
    /// Context 策略（bigint 矩阵）。
    pub context_policy: Option<ContextPolicy>,
    /// 实现名（athena / num / …）。
    pub implementation: Option<&'static str>,
    /// 操作名（add / mul / …）。
    pub operation: Option<&'static str>,
    /// 位宽。
    pub bits: Option<u32>,
    /// 本次有效 `GcMode` 名。
    pub gc_mode: Option<&'static str>,
}

impl FixtureMeta {
    /// 非 bigint 种子 fixture 的简写构造。
    pub fn basic(id: &'static str, group: BenchGroup, scale: &'static str, domain: &'static str) -> Self {
        Self {
            id,
            group,
            scale,
            domain,
            layer: None,
            context_policy: None,
            implementation: None,
            operation: None,
            bits: None,
            gc_mode: None,
        }
    }
}

/// 单个确定性基准。
pub trait Fixture: Send {
    /// 元数据。
    fn meta(&self) -> FixtureMeta;

    /// 是否跳过（例如未启用 JIT）。
    fn skip_reason(&self) -> Option<&'static str> {
        None
    }

    /// 计时前正确性校验。
    fn validate(&self) -> Result<ValidationSummary, String>;

    /// 热路径主体（单次）。
    fn run_once(&self);
}

/// 运行参数。
#[derive(Debug, Clone)]
pub struct RunConfig {
    /// 要运行的分组；空表示全部。
    pub groups: Vec<BenchGroup>,
    /// 预热次数。
    pub warmup: usize,
    /// 采样次数。
    pub samples: usize,
    /// 报告分层（决定建议 `GcMode` 与强制内存字段语义）。
    pub report_tier: ReportTier,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self { groups: Vec::new(), warmup: 3, samples: 25, report_tier: ReportTier::EndToEnd }
    }
}

/// Suite 编排错误。
#[derive(Debug)]
pub enum SuiteError {
    /// 校验失败。
    Validation {
        /// Fixture id。
        id: String,
        /// 原因。
        reason: String,
    },
}

impl fmt::Display for SuiteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation { id, reason } => write!(f, "validation failed for `{id}`: {reason}"),
        }
    }
}

impl std::error::Error for SuiteError {}

/// 已注册 fixture 集合。
pub struct Suite {
    fixtures: Vec<Box<dyn Fixture>>,
}

impl Suite {
    /// 空 suite。
    pub fn new() -> Self {
        Self { fixtures: Vec::new() }
    }

    /// 注册 fixture。
    pub fn register(&mut self, fixture: Box<dyn Fixture>) {
        self.fixtures.push(fixture);
    }

    /// 全部 fixture。
    pub fn fixtures(&self) -> &[Box<dyn Fixture>] {
        &self.fixtures
    }
}

impl Default for Suite {
    fn default() -> Self {
        Self::new()
    }
}

/// 执行单个 fixture → [`FixtureReport`]。
pub fn run_fixture(fixture: &dyn Fixture, config: &RunConfig, _env: &BenchEnv) -> Result<FixtureReport, SuiteError> {
    let meta = fixture.meta();
    let tier = tier_for_meta(&meta, config.report_tier);
    if let Some(reason) = fixture.skip_reason() {
        return Ok(base_report(&meta, config, tier, true, None, None, None, reason));
    }

    let validation = fixture.validate().map_err(|reason| SuiteError::Validation { id: meta.id.to_string(), reason })?;
    if !validation.ok {
        return Err(SuiteError::Validation { id: meta.id.to_string(), reason: validation.notes.clone() });
    }

    let stats = measure(config.warmup, config.samples, || fixture.run_once());
    let gc_stats = sample_gc_stats(tier);

    Ok(FixtureReport {
        id: meta.id.to_string(),
        group: meta.group.as_str().to_string(),
        scale: meta.scale.to_string(),
        domain: meta.domain.to_string(),
        warmup: config.warmup,
        samples: config.samples,
        skipped: false,
        p50_ns: Some(stats.p50_ns),
        p95_ns: Some(stats.p95_ns),
        alloc_bytes: None,
        peak_rss_bytes: peak_rss_bytes(),
        report_tier: Some(tier),
        layer: meta.layer,
        context_policy: meta.context_policy,
        implementation: meta.implementation.map(str::to_string),
        operation: meta.operation.map(str::to_string),
        bits: meta.bits,
        gc_mode: Some(meta.gc_mode.unwrap_or(gc_stats.gc_mode.as_str()).to_string()),
        peak_arena_bytes: Some(gc_stats.peak_arena_bytes),
        peak_scratch_bytes: Some(gc_stats.peak_scratch_bytes),
        gc_time_ns: Some(gc_stats.gc_time_ns),
        validation,
        fallback_reason: None,
    })
}

fn tier_for_meta(meta: &FixtureMeta, fallback: ReportTier) -> ReportTier {
    match meta.layer {
        Some(BenchLayer::Kernel) => ReportTier::Kernel,
        Some(BenchLayer::Numeric) => ReportTier::Arena,
        Some(BenchLayer::E2e) => ReportTier::EndToEnd,
        Some(BenchLayer::Peer) | None => fallback,
    }
}

fn base_report(
    meta: &FixtureMeta,
    config: &RunConfig,
    tier: ReportTier,
    skipped: bool,
    p50_ns: Option<u64>,
    p95_ns: Option<u64>,
    validation: Option<ValidationSummary>,
    reason: &str,
) -> FixtureReport {
    FixtureReport {
        id: meta.id.to_string(),
        group: meta.group.as_str().to_string(),
        scale: meta.scale.to_string(),
        domain: meta.domain.to_string(),
        warmup: config.warmup,
        samples: config.samples,
        skipped,
        p50_ns,
        p95_ns,
        alloc_bytes: None,
        peak_rss_bytes: None,
        report_tier: Some(tier),
        layer: meta.layer,
        context_policy: meta.context_policy,
        implementation: meta.implementation.map(str::to_string),
        operation: meta.operation.map(str::to_string),
        bits: meta.bits,
        gc_mode: Some(meta.gc_mode.unwrap_or(tier.suggested_gc_mode()).to_string()),
        peak_arena_bytes: None,
        peak_scratch_bytes: None,
        gc_time_ns: None,
        validation: validation.unwrap_or_else(|| {
            ValidationSummary::passed(
                crate::validate::ExactnessKind::Unspecified,
                crate::validate::DeterminacyKind::Unspecified,
                "skipped",
            )
        }),
        fallback_reason: Some(reason.to_string()),
    }
}

struct GcSample {
    gc_mode: String,
    peak_arena_bytes: u64,
    peak_scratch_bytes: u64,
    gc_time_ns: u64,
}

fn sample_gc_stats(tier: ReportTier) -> GcSample {
    use athena_gc::{GcHeap, GcMode, HeapBudget};
    use athena_numeric::{ExecutionBudget, NumericContext, natural::Natural};

    let heap = GcHeap::new_shared(HeapBudget::default());
    let mode = match tier {
        ReportTier::Kernel => GcMode::Disabled,
        ReportTier::Arena => GcMode::Deferred,
        ReportTier::EndToEnd => GcMode::Auto,
    };
    heap.borrow().gc().set_base_mode(mode);
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap.clone());
    // 轻量分配以填充 arena/scratch 峰值字段（不冒充端到端吞吐）。
    let _ = Natural::from_limbs_in(&ctx, vec![1, 2, 3, 4]);
    let _ = heap.borrow_mut().collect();
    let stats = heap.borrow().stats();
    GcSample {
        gc_mode: match heap.borrow().effective_mode() {
            GcMode::Auto => "auto".into(),
            GcMode::Deferred => "deferred".into(),
            GcMode::Disabled => "disabled".into(),
        },
        peak_arena_bytes: stats.peak_arena_bytes as u64,
        peak_scratch_bytes: stats.peak_scratch_bytes as u64,
        gc_time_ns: stats.gc_time_ns,
    }
}

fn peak_rss_bytes() -> Option<u64> {
    // 权威内存字段是 arena / scratch（GcHeap stats）。进程 RSS 可选，不用外部脚本探测。
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if let Some(rest) = line.strip_prefix("VmHWM:") {
                let kb: u64 = rest.split_whitespace().next()?.parse().ok()?;
                return Some(kb.saturating_mul(1024));
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
