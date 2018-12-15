//! 连通分量（L1 · 调 `athena-graph`）。

use athena_graph::connected_components;

use super::{
    object::GraphObject,
    property::{GraphCertificate, GraphPropertyKind, GraphPropertyResult, GraphPropertyState},
    result::ConnectedComponentsResult,
};

/// 计算弱连通分量标签。
pub fn connected_components_l1(graph: &GraphObject) -> ConnectedComponentsResult {
    let inner = graph.to_athena_graph();
    let labels = connected_components(&inner);
    let component_count = labels.iter().copied().collect::<std::collections::HashSet<_>>().len() as u64;
    let property = GraphPropertyResult {
        kind: GraphPropertyKind::ConnectedComponents,
        state: GraphPropertyState::ProvenTrue,
        value: component_count,
        certificate: Some(GraphCertificate::TraversalWitness {
            algorithm: "connected_components",
            visited_count: inner.node_count(),
        }),
        algorithm: "connected_components",
    };
    ConnectedComponentsResult { labels, component_count, property }
}
