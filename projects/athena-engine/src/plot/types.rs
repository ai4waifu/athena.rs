//! 采样合同类型（中立载荷，无 Scene / PlotSpec）。

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// 1D 实区间域。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SampleDomain {
    /// 左端点。
    pub start: f64,
    /// 右端点。
    pub end: f64,
}

impl SampleDomain {
    /// 构造闭区间。
    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }
}

/// 采样策略（均匀网格 + 间断检测 + 协作取消）。
#[derive(Debug, Clone)]
pub struct SamplingPolicy {
    /// 最大样本数（含端点）。
    pub max_samples: u32,
    /// 相对跳跃阈值：`|Δy| > rel * (1+|y0|+|y1|)` 时在后一点插入 gap（`None` 关闭）。
    pub discontinuity_rel: Option<f64>,
    /// 协作取消标志；采样循环中若为 true 则返回 [`DiagnosticCode::SamplingCancelled`](athena_types::DiagnosticCode::SamplingCancelled)。
    pub cancel: Option<Arc<AtomicBool>>,
}

impl Default for SamplingPolicy {
    fn default() -> Self {
        Self { max_samples: 128, discontinuity_rel: Some(32.0), cancel: None }
    }
}

impl PartialEq for SamplingPolicy {
    fn eq(&self, other: &Self) -> bool {
        self.max_samples == other.max_samples
            && self.discontinuity_rel == other.discontinuity_rel
            && match (&self.cancel, &other.cancel) {
                (None, None) => true,
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                _ => false,
            }
    }
}

impl SamplingPolicy {
    /// 仅指定样本数（默认间断阈值，无取消）。
    pub fn samples(max_samples: u32) -> Self {
        Self { max_samples, ..Self::default() }
    }

    pub(crate) fn is_cancelled(&self) -> bool {
        self.cancel.as_ref().is_some_and(|f| f.load(Ordering::Relaxed))
    }
}

/// 单个采样点。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplePoint {
    /// 自变量。
    pub x: f64,
    /// 因变量（`valid == false` 时可为占位）。
    pub y: f64,
    /// 是否为有限有效值。
    pub valid: bool,
}

/// 采样曲线。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SampledCurve {
    /// 按 `x` 升序排列的点。
    pub points: Vec<SamplePoint>,
    /// 间隙起点下标（该点无效，或与前一有效点之间不得连线）。
    pub gaps: Vec<usize>,
}
