//! 节点 / 边身份。

/// 存储 wire 节点身份（`u64` newtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct NodeId(pub u64);

/// 存储 wire 边身份（`u64` newtype）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct EdgeId(pub u64);
