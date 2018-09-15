//! 求解器标识与元数据。

pub use crate::mgraph::SolverId;

/// 求解器元数据。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SolverMetadata {
    /// 算法名。
    pub algorithm: String,
    /// 是否完整。
    pub complete: bool,
}
