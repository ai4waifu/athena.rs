//! 非负权最短路（Dijkstra）。

use std::{cmp::Ordering, collections::BinaryHeap};

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    object::{GraphNodeId, GraphObject, MemoryGraph, WeightDomain},
    property::{GraphCertificate, GraphPropertyKind, GraphPropertyResult, GraphPropertyState},
    result::ShortestPathResult,
};

#[derive(Eq, PartialEq)]
struct State {
    cost: u64,
    node: GraphNodeId,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost)
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 非负权 Dijkstra；无权域边权按 1 计。
pub fn shortest_path_non_negative(graph: &GraphObject, source: GraphNodeId, target: GraphNodeId) -> Result<ShortestPathResult, Diagnostic> {
    validate_weight_domain(graph)?;
    let inner = graph.to_athena_graph();
    let n = graph.node_count();
    if source.0 >= n || target.0 >= n {
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "graph_theory").detail("field", "node"));
    }
    let unit_weight = matches!(graph.semantics.weight_domain, WeightDomain::Unweighted);
    let mut dist = vec![u64::MAX; n as usize];
    let mut prev: Vec<Option<GraphNodeId>> = vec![None; n as usize];
    dist[source.0 as usize] = 0;
    let mut heap = BinaryHeap::new();
    heap.push(State { cost: 0, node: source });
    let mut relaxations = 0u64;
    while let Some(State { cost, node }) = heap.pop() {
        if cost > dist[node.0 as usize] {
            continue;
        }
        if node == target {
            break;
        }
        for next in inner.out_neighbors(node) {
            let w = edge_weight(&graph.memory, node, next, unit_weight)?;
            let next_cost = cost.checked_add(w).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::NumericResourceLimit).detail("domain", "graph_theory").detail("field", "distance")
            })?;
            relaxations += 1;
            let idx = next.0 as usize;
            if next_cost < dist[idx] {
                dist[idx] = next_cost;
                prev[idx] = Some(node);
                heap.push(State { cost: next_cost, node: next });
            }
        }
    }

    let edge_weights = collect_edge_weights(&graph.memory, unit_weight);
    let dist_opt: Vec<Option<u64>> = dist.iter().map(|&d| if d == u64::MAX { None } else { Some(d) }).collect();
    let dual = GraphCertificate::ShortestPathDual {
        algorithm: "dijkstra",
        source,
        target,
        dist: dist_opt,
        pred: prev.clone(),
        edge_weights,
        nonnegative_assumed: true,
        relaxations,
    };

    if dist[target.0 as usize] == u64::MAX {
        return Ok(ShortestPathResult::Unreachable {
            property: GraphPropertyResult {
                kind: GraphPropertyKind::Reachability,
                state: GraphPropertyState::ProvenFalse,
                value: (),
                certificate: Some(dual),
                strength: super::property::CertificateStrength::Summary,
                algorithm: "dijkstra",
                snapshot: graph.snapshot.clone(),
            },
        });
    }
    let mut path = vec![target];
    let mut cur = target;
    while cur != source {
        let Some(p) = prev[cur.0 as usize]
        else {
            return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "graph_theory").detail("field", "path_reconstruct"));
        };
        path.push(p);
        cur = p;
    }
    path.reverse();
    Ok(ShortestPathResult::Found {
        distance: dist[target.0 as usize],
        path,
        property: GraphPropertyResult {
            kind: GraphPropertyKind::Reachability,
            state: GraphPropertyState::ProvenTrue,
            value: dist[target.0 as usize],
            certificate: Some(dual),
            strength: super::property::CertificateStrength::Summary,
            algorithm: "dijkstra",
            snapshot: graph.snapshot.clone(),
        },
    })
}

fn collect_edge_weights(memory: &MemoryGraph, unit_weight: bool) -> Vec<(GraphNodeId, GraphNodeId, u64)> {
    memory.edges.iter().map(|(s, t, w)| (*s, *t, if unit_weight { 1 } else { *w })).collect()
}

fn validate_weight_domain(graph: &GraphObject) -> Result<(), Diagnostic> {
    match graph.semantics.weight_domain {
        WeightDomain::Unweighted | WeightDomain::NonNegativeInteger => Ok(()),
    }
}

fn edge_weight(memory: &MemoryGraph, source: GraphNodeId, target: GraphNodeId, unit_weight: bool) -> Result<u64, Diagnostic> {
    if unit_weight {
        return Ok(1);
    }
    memory
        .edges
        .iter()
        .find(|(s, t, _)| *s == source && *t == target)
        .map(|(_, _, w)| *w)
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "graph_theory").detail("field", "edge"))
}
