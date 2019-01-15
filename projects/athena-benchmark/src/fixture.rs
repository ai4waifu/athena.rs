//! Fixture 合同与 suite 编排。

use std::fmt;

use crate::{env::BenchEnv, report::FixtureReport, timing::measure, validate::ValidationSummary};

/// 基准分组标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BenchGroup {
    /// ExactInteger / ExactRational / promotion / 机器实数等。
    Numeric,
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
}

/// 单个确定性基准。
pub trait Fixture: Send + Sync {
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
}

impl Default for RunConfig {
    fn default() -> Self {
        Self { groups: Vec::new(), warmup: 3, samples: 25 }
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
    if let Some(reason) = fixture.skip_reason() {
        return Ok(FixtureReport {
            id: meta.id.to_string(),
            group: meta.group.as_str().to_string(),
            scale: meta.scale.to_string(),
            domain: meta.domain.to_string(),
            warmup: config.warmup,
            samples: config.samples,
            skipped: true,
            p50_ns: None,
            p95_ns: None,
            alloc_bytes: None,
            peak_rss_bytes: None,
            validation: ValidationSummary::passed(
                crate::validate::ExactnessKind::Unspecified,
                crate::validate::DeterminacyKind::Unspecified,
                "skipped",
            ),
            fallback_reason: Some(reason.to_string()),
        });
    }

    let validation = fixture.validate().map_err(|reason| SuiteError::Validation { id: meta.id.to_string(), reason })?;
    if !validation.ok {
        return Err(SuiteError::Validation { id: meta.id.to_string(), reason: validation.notes.clone() });
    }

    let stats = measure(config.warmup, config.samples, || fixture.run_once());

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
        validation,
        fallback_reason: None,
    })
}

fn peak_rss_bytes() -> Option<u64> {
    #[cfg(target_os = "windows")]
    {
        // 尽力通过 PowerShell 读取；失败则返回 null（不挡 CI）。
        let out = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", "(Get-Process -Id $PID).WorkingSet64"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        s.parse().ok()
    }
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
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        None
    }
}
