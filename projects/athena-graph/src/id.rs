//! 节点 / 边身份与图修订号。

/// 存储 wire 节点身份（`u64` newtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeId(pub u64);

/// 存储 wire 边身份（`u64` newtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct EdgeId(pub u64);

/// 图结构修订号（`add_node` / `add_edge` 单调递增）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub struct GraphRevision(pub u64);
