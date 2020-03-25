//! 图性质状态与结果（禁止裸 `bool`）。

use athena_graph::GraphSnapshot;

use super::{
    object::{GraphNodeId, WeightDomain},
    result::SpanningEdge,
};

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

/// 证书强度（决定可否 exact M-Graph admission）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CertificateStrength {
    /// 算法输出摘要，**禁止** exact admission。
    Summary,
    /// 已通过独立 verifier 重放。
    IndependentlyVerified,
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
    /// 森林 / 树。
    Forest,
    /// 有向无环 / 拓扑序。
    Dag,
    /// 桥。
    Bridge,
    /// 割点。
    Articulation,
}

/// 可独立验证的图论证书（摘要级变体仍保留，但不得 admission）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub enum GraphCertificate {
    /// 遍历见证（摘要；不可 admission）。
    TraversalWitness {
        /// 算法名。
        algorithm: &'static str,
        /// 访问节点数。
        visited_count: u64,
    },
    /// 连通分量划分。
    ComponentPartition {
        /// 算法名。
        algorithm: &'static str,
        /// 每节点分量代表。
        labels: Vec<GraphNodeId>,
    },
    /// SCC 划分。
    SccPartition {
        /// 算法名。
        algorithm: &'static str,
        /// 每节点 SCC 代表。
        labels: Vec<GraphNodeId>,
    },
    /// 最短路 dual / 前驱树（可验松弛不等式）。
    ShortestPathDual {
        /// 算法名。
        algorithm: &'static str,
        /// 源。
        source: GraphNodeId,
        /// 目标（查询终点）。
        target: GraphNodeId,
        /// 距离（不可达为 `None`）。
        dist: Vec<Option<u64>>,
        /// 前驱。
        pred: Vec<Option<GraphNodeId>>,
        /// 边权快照 `(u,v,w)`。
        edge_weights: Vec<(GraphNodeId, GraphNodeId, u64)>,
        /// 是否假设非负权。
        nonnegative_assumed: bool,
        /// 松弛步数（诊断）。
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
    /// Kruskal 森林摘要（不可 admission）。
    KruskalForest {
        /// 算法名。
        algorithm: &'static str,
        /// 选中边数。
        edge_count: u64,
        /// 森林中树/分量数。
        tree_count: u64,
    },
    /// MST/MSF cut 证书。
    MstCut {
        /// 算法名。
        algorithm: &'static str,
        /// 选中边。
        edges: Vec<SpanningEdge>,
        /// 总权。
        total_weight: u64,
        /// 树个数。
        tree_count: u64,
        /// 权重域。
        weight_domain: WeightDomain,
    },
    /// 拓扑序。
    TopologicalOrder {
        /// 算法名。
        algorithm: &'static str,
        /// 节点序。
        order: Vec<GraphNodeId>,
    },
    /// 有向环。
    DirectedCycle {
        /// 算法名。
        algorithm: &'static str,
        /// 环（首尾闭合）。
        cycle: Vec<GraphNodeId>,
    },
    /// 森林证明（边集无环 + 分量树）。
    ForestProof {
        /// 算法名。
        algorithm: &'static str,
        /// 边数。
        edge_count: u64,
        /// 分量数。
        component_count: u64,
        /// 是否恰为一棵树（单连通分量森林）。
        is_tree: bool,
    },
    /// 桥集。
    BridgeSet {
        /// 算法名。
        algorithm: &'static str,
        /// 桥边（无向规范化 `source ≤ target`）。
        bridges: Vec<(GraphNodeId, GraphNodeId)>,
    },
    /// 割点集。
    ArticulationSet {
        /// 算法名。
        algorithm: &'static str,
        /// 割点。
        nodes: Vec<GraphNodeId>,
    },
}

impl GraphCertificate {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::TraversalWitness { algorithm, visited_count } => {
                Self::TraversalWitness { algorithm: *algorithm, visited_count: *visited_count }
            }
            Self::ComponentPartition { algorithm, labels } => Self::ComponentPartition { algorithm: *algorithm, labels: labels.clone() },
            Self::SccPartition { algorithm, labels } => Self::SccPartition { algorithm: *algorithm, labels: labels.clone() },
            Self::ShortestPathDual { algorithm, source, target, dist, pred, edge_weights, nonnegative_assumed, relaxations } => {
                Self::ShortestPathDual {
                    algorithm: *algorithm,
                    source: *source,
                    target: *target,
                    dist: dist.clone(),
                    pred: pred.clone(),
                    edge_weights: edge_weights.clone(),
                    nonnegative_assumed: *nonnegative_assumed,
                    relaxations: *relaxations,
                }
            }
            Self::BipartiteColoring { algorithm, left, right } => {
                Self::BipartiteColoring { algorithm: *algorithm, left: left.clone(), right: right.clone() }
            }
            Self::OddCycle { algorithm, cycle } => Self::OddCycle { algorithm: *algorithm, cycle: cycle.clone() },
            Self::KruskalForest { algorithm, edge_count, tree_count } => {
                Self::KruskalForest { algorithm: *algorithm, edge_count: *edge_count, tree_count: *tree_count }
            }
            Self::MstCut { algorithm, edges, total_weight, tree_count, weight_domain } => Self::MstCut {
                algorithm: *algorithm,
                edges: edges.clone(),
                total_weight: *total_weight,
                tree_count: *tree_count,
                weight_domain: *weight_domain,
            },
            Self::TopologicalOrder { algorithm, order } => Self::TopologicalOrder { algorithm: *algorithm, order: order.clone() },
            Self::DirectedCycle { algorithm, cycle } => Self::DirectedCycle { algorithm: *algorithm, cycle: cycle.clone() },
            Self::ForestProof { algorithm, edge_count, component_count, is_tree } => {
                Self::ForestProof { algorithm: *algorithm, edge_count: *edge_count, component_count: *component_count, is_tree: *is_tree }
            }
            Self::BridgeSet { algorithm, bridges } => Self::BridgeSet { algorithm: *algorithm, bridges: bridges.clone() },
            Self::ArticulationSet { algorithm, nodes } => Self::ArticulationSet { algorithm: *algorithm, nodes: nodes.clone() },
        }
    }
}

/// 带状态与证书的性质结果。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]（`T: Copy`）。
#[derive(Debug, PartialEq)]
pub struct GraphPropertyResult<T> {
    /// 性质种类。
    pub kind: GraphPropertyKind,
    /// 判定状态。
    pub state: GraphPropertyState,
    /// 性质载荷（如分量数、距离）。
    pub value: T,
    /// 可选证书。
    pub certificate: Option<GraphCertificate>,
    /// 证书强度。
    pub strength: CertificateStrength,
    /// 产出算法。
    pub algorithm: &'static str,
    /// 所证图快照。
    pub snapshot: GraphSnapshot,
}

impl<T: Copy> GraphPropertyResult<T> {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            kind: self.kind,
            state: self.state,
            value: self.value,
            certificate: self.certificate.as_ref().map(GraphCertificate::owning_copy),
            strength: self.strength,
            algorithm: self.algorithm,
            snapshot: self.snapshot,
        }
    }
}

impl<T> GraphPropertyResult<T> {
    /// 已证精确结果（默认摘要强度；verifier 通过后升级）。
    pub fn proven(kind: GraphPropertyKind, value: T, algorithm: &'static str, certificate: GraphCertificate, snapshot: GraphSnapshot) -> Self {
        Self {
            kind,
            state: GraphPropertyState::ProvenTrue,
            value,
            certificate: Some(certificate),
            strength: CertificateStrength::Summary,
            algorithm,
            snapshot,
        }
    }

    /// 已证不成立。
    pub fn disproven(
        kind: GraphPropertyKind,
        value: T,
        algorithm: &'static str,
        certificate: GraphCertificate,
        snapshot: GraphSnapshot,
    ) -> Self {
        Self {
            kind,
            state: GraphPropertyState::ProvenFalse,
            value,
            certificate: Some(certificate),
            strength: CertificateStrength::Summary,
            algorithm,
            snapshot,
        }
    }

    /// 标记为已独立验证。
    pub fn mark_verified(mut self) -> Self {
        self.strength = CertificateStrength::IndependentlyVerified;
        self
    }

    /// Summary 证书禁止 exact M-Graph admission。
    pub fn allows_exact_admission(&self) -> bool {
        matches!(self.strength, CertificateStrength::IndependentlyVerified)
            && matches!(self.state, GraphPropertyState::ProvenTrue | GraphPropertyState::ProvenFalse)
    }
}
