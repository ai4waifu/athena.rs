//! 求解器标识与元数据（调度侧，不含数学覆盖承诺）。

pub use crate::reasoning::mgraph::CapabilityProviderId;

/// 求解器元数据。
///
/// 数学覆盖状态属于 [`crate::domains::solve::CoverageStatus`]，不在此用 `complete: bool` 表示。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct SolverMetadata {
    /// 算法名（机器标识）。
    pub algorithm: String,
}

impl SolverMetadata {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            algorithm: self.algorithm.clone(),
        }
    }
}
