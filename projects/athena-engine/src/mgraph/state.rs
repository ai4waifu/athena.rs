//! M-Graph 状态容器。

use super::{operational::OperationalState, semantic::SemanticCore};

/// M-Graph 状态：semantic core 与 operational 层分离。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MGraphState {
    /// 单调 verified claims + 派生索引。
    pub semantic: SemanticCore,
    /// frontier · 非语义缓存 · 暂存结构。
    pub operational: OperationalState,
}

impl MGraphState {
    /// 空状态。
    pub fn new() -> Self {
        Self::default()
    }
}
