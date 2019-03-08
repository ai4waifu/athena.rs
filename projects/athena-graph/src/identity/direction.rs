//! 图方向（结构层）。

/// 图方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphDirection {
    /// 有向。
    Directed,
    /// 无向。
    Undirected,
}
