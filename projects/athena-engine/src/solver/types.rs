//! 求解器标识与元数据（调度侧，不含数学覆盖承诺）。

pub use crate::mgraph::SolverId;

/// 求解器元数据。
///
/// 数学覆盖状态属于 [`crate::solve::CoverageStatus`]，不在此用 `complete: bool` 表示。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SolverMetadata {
    /// 算法名（机器标识）。
    pub algorithm: String,
}
