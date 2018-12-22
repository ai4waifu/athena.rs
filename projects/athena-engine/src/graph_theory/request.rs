//! 图论强类型请求。

use super::object::{GraphNodeId, GraphObject};

/// 图论域请求（禁止字符串算法名）。
#[derive(Debug, Clone, PartialEq)]
pub enum GraphTheoryRequest {
    /// 弱连通分量（有向图按无向解释，同 `athena-graph` L0）。
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
