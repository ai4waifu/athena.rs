//! 采样合同类型（中立载荷，无 Scene / PlotSpec）。

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

/// 采样策略（第一刀：均匀网格）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SamplingPolicy {
    /// 最大样本数（含端点）。
    pub max_samples: u32,
}

impl Default for SamplingPolicy {
    fn default() -> Self {
        Self { max_samples: 128 }
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
