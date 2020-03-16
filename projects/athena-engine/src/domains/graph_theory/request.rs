//! 图论强类型请求。

use super::object::{GraphNodeId, GraphObject};

/// 图论域请求（禁止字符串算法名）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum GraphTheoryRequest {
    /// 弱连通分量（有向图按无向解释，同 `athena-graph` 基础算法）。
    ConnectedComponents {
        /// 输入图。
        graph: GraphObject,
    },
    /// 强连通分量（仅有向图）。
    StronglyConnectedComponents {
        /// 输入图。
        graph: GraphObject,
    },
    /// 二部性判定（无向图；有向图按底层无向邻接解释）。
    Bipartite {
        /// 输入图。
        graph: GraphObject,
    },
    /// 最小生成森林（Kruskal；无权边权按 1）。
    MinimumSpanningForest {
        /// 输入图。
        graph: GraphObject,
    },
    /// 非负权最短路（无权视为边权 1）。
    ShortestPath {
        /// 输入图。
        graph: GraphObject,
        /// 源。
        source: GraphNodeId,
        /// 目标。
        target: GraphNodeId,
    },
}

impl GraphTheoryRequest {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::ConnectedComponents { graph } => Self::ConnectedComponents { graph: graph.owning_copy() },
            Self::StronglyConnectedComponents { graph } => Self::StronglyConnectedComponents { graph: graph.owning_copy() },
            Self::Bipartite { graph } => Self::Bipartite { graph: graph.owning_copy() },
            Self::MinimumSpanningForest { graph } => Self::MinimumSpanningForest { graph: graph.owning_copy() },
            Self::ShortestPath { graph, source, target } => {
                Self::ShortestPath { graph: graph.owning_copy(), source: *source, target: *target }
            }
        }
    }
}
