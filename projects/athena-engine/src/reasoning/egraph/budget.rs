//! 作用域内饱和的预算与停止原因。

/// 单次饱和运行的硬上限。零表示「禁用 / 立即停止」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SaturationBudget {
    /// 等价类数量上限。
    pub max_eclasses: u32,
    /// enode 数量上限。
    pub max_enodes: u32,
    /// 重写 / 合并迭代次数上限。
    pub max_iterations: u32,
    /// 本次运行可发出的候选并查集合并次数上限。
    pub max_candidate_unions: u32,
}

impl Default for SaturationBudget {
    fn default() -> Self {
        Self { max_eclasses: 1_024, max_enodes: 4_096, max_iterations: 64, max_candidate_unions: 512 }
    }
}

impl SaturationBudget {
    /// 冒烟 / 契约测试用的小预算。
    pub const fn smoke() -> Self {
        Self { max_eclasses: 32, max_enodes: 128, max_iterations: 8, max_candidate_unions: 16 }
    }
}

/// 饱和停止原因（绝不会「永远跑下去」）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaturationStopReason {
    /// 在预算内到达不动点（无待处理工作）。
    FixedPoint,
    /// 触达迭代上限。
    IterationBudget,
    /// 触达 eclass / enode / union 资源上限。
    ResourceBudget,
    /// 调用方取消（预留钩子）。
    Cancelled,
}
