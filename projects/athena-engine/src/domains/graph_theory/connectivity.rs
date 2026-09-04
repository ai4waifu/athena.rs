//! 连通分量与强连通分量（调 `athena-graph`）。

use athena_graph::primitives::{connected_components, strongly_connected_components};
use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    object::GraphObject,
    property::{GraphCertificate, GraphPropertyKind, GraphPropertyResult},
    result::{ConnectedComponentsResult, StronglyConnectedComponentsResult},
};

/// 计算弱连通分量标签。
pub fn connected_components_l1(graph: &GraphObject) -> ConnectedComponentsResult {
    let inner = graph.to_athena_graph();
    let labels = connected_components(&inner);
    let component_count = labels.iter().copied().collect::<std::collections::HashSet<_>>().len() as u64;
    let property = GraphPropertyResult::proven(
        GraphPropertyKind::ConnectedComponents,
        component_count,
        "connected_components",
        GraphCertificate::ComponentPartition { algorithm: "connected_components", labels: labels.clone() },
        graph.snapshot.clone(),
    );
    ConnectedComponentsResult { labels, component_count, property }
}

/// 计算强连通分量标签（仅有向图）。
pub fn strongly_connected_components_l1(graph: &GraphObject) -> Result<StronglyConnectedComponentsResult, Diagnostic> {
    let inner = graph.to_athena_graph();
    let labels = strongly_connected_components(&inner).map_err(|_| {
        Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "graph_theory")
            .detail("operation", "StronglyConnectedComponents")
            .detail("reason", "directed_only")
    })?;
    let component_count = labels.iter().copied().collect::<std::collections::HashSet<_>>().len() as u64;
    let property = GraphPropertyResult::proven(
        GraphPropertyKind::StrongConnectivity,
        component_count,
        "kosaraju",
        GraphCertificate::SccPartition { algorithm: "kosaraju", labels: labels.clone() },
        graph.snapshot.clone(),
    );
    Ok(StronglyConnectedComponentsResult { labels, component_count, property })
}
