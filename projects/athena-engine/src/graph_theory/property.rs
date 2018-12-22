//! 图性质状态与结果（禁止裸 `bool`）。

use super::object::GraphNodeId;

/// 图性质判定状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphPropertyState {
    /// 已证成立。
    ProvenTrue,
    /// 已证不成立。
    ProvenFalse,
    /// 条件成立（假设未完全验证）。
    Conditional,
    /// 高概率但未证。
    Probable,
    /// 未知。
    Unknown,
    /// 资源或预算截断。
    Incomplete,
}

/// 性质种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GraphPropertyKind {
    /// 弱连通 / 连通分量。
    ConnectedComponents,
    /// 强连通分量。
    StrongConnectivity,
    /// 二部性。
    Bipartiteness,
    /// 最小生成森林。
    SpanningForest,
    /// 两点间最短路存在性。
    Reachability,
}

/// 轻量证书（完整 verifier 后续扩展）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphCertificate {
    /// 遍历见证（BFS/DFS 顺序摘要）。
    TraversalWitness {
        /// 算法名。
        algorithm: &'static str,
        /// 访问节点数。
        visited_count: u64,
    },
    /// 最短路见证：前驱链可重放。
    ShortestPathWitness {
        /// 算法名。
        algorithm: &'static str,
        /// 松弛步数。
        relaxations: u64,
    },
    /// 二染色见证。
    BipartiteColoring {
        /// 算法名。
        algorithm: &'static str,
        /// 一侧节点。
        left: Vec<GraphNodeId>,
        /// 另一侧节点。
        right: Vec<GraphNodeId>,
    },
    /// 奇环反例（非二部）。
    OddCycle {
        /// 算法名。
        algorithm: &'static str,
        /// 奇环节点序列（首尾闭合）。
        cycle: Vec<GraphNodeId>,
    },
    /// Kruskal 森林见证。
    KruskalForest {
        /// 算法名。
        algorithm: &'static str,
        /// 选中边数。
        edge_count: u64,
        /// 森林中树/分量数。
        tree_count: u64,
    },
}

/// 带状态与证书的性质结果。
#[derive(Debug, Clone, PartialEq)]
pub struct GraphPropertyResult<T> {
    /// 性质种类。
    pub kind: GraphPropertyKind,
    /// 判定状态。
    pub state: GraphPropertyState,
    /// 性质载荷（如分量数、距离）。
    pub value: T,
    /// 可选证书。
    pub certificate: Option<GraphCertificate>,
    /// 产出算法。
    pub algorithm: &'static str,
}

impl<T> GraphPropertyResult<T> {
    /// 已证精确结果。
    pub fn proven(kind: GraphPropertyKind, value: T, algorithm: &'static str, certificate: GraphCertificate) -> Self {
        Self { kind, state: GraphPropertyState::ProvenTrue, value, certificate: Some(certificate), algorithm }
    }

    /// 已证不成立。
    pub fn disproven(kind: GraphPropertyKind, value: T, algorithm: &'static str, certificate: GraphCertificate) -> Self {
        Self { kind, state: GraphPropertyState::ProvenFalse, value, certificate: Some(certificate), algorithm }
    }
}
