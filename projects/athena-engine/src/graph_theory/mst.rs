//! 最小生成森林（L1 · Kruskal）。

use athena_graph::UnionFind;
use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    object::{GraphNodeId, GraphObject, WeightDomain},
    property::{GraphCertificate, GraphPropertyKind, GraphPropertyResult},
    result::{MinimumSpanningForestResult, SpanningEdge},
};

/// Kruskal 最小生成森林；无权边权按 1。
pub fn minimum_spanning_forest_l1(graph: &GraphObject) -> Result<MinimumSpanningForestResult, Diagnostic> {
    match graph.semantics.weight_domain {
        WeightDomain::Unweighted | WeightDomain::NonNegativeInteger => {}
    }
    let n = graph.node_count() as usize;
    let unit = matches!(graph.semantics.weight_domain, WeightDomain::Unweighted);
    let mut edges: Vec<(u64, GraphNodeId, GraphNodeId)> = graph
        .memory
        .edges
        .iter()
        .filter(|(s, t, _)| s.0 < n as u64 && t.0 < n as u64 && s != t)
        .map(|(s, t, w)| {
            let weight = if unit { 1 } else { *w };
            let (a, b) = if s.0 <= t.0 { (*s, *t) } else { (*t, *s) };
            (weight, a, b)
        })
        .collect();
    edges.sort_unstable_by_key(|(w, a, b)| (*w, a.0, b.0));
    edges.dedup_by(|a, b| a.1 == b.1 && a.2 == b.2 && a.0 == b.0);

    let mut uf = UnionFind::new(n);
    let mut selected = Vec::new();
    let mut total_weight = 0u64;
    for (w, a, b) in edges {
        if uf.union(a.0 as usize, b.0 as usize) {
            total_weight = total_weight.checked_add(w).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericResourceLimit)
                    .detail("domain", "graph_theory")
                    .detail("operation", "MinimumSpanningForest")
            })?;
            selected.push(SpanningEdge { source: a, target: b, weight: w });
        }
    }
    let tree_count = uf.set_count() as u64;
    let property = GraphPropertyResult::proven(
        GraphPropertyKind::SpanningForest,
        total_weight,
        "kruskal",
        GraphCertificate::MstCut {
            algorithm: "kruskal",
            edges: selected.clone(),
            total_weight,
            tree_count,
            weight_domain: graph.semantics.weight_domain,
        },
        graph.snapshot.clone(),
    );
    Ok(MinimumSpanningForestResult { edges: selected, total_weight, tree_count, property })
}
