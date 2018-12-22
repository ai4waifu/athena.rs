//! 图论与 `DomainRequest::GraphTheory` 集成测试（G3–G4：SCC · Bipartite · MST）。

use athena_engine::{
    BipartiteResult, ConnectedComponentsResult, DomainRequest, DomainResult, GraphCertificate, GraphHandle, GraphObject,
    GraphPropertyState, GraphSemantics, GraphTheoryRequest, GraphTheoryResult, GraphTheoryValue, MinimumSpanningForestResult,
    ShortestPathResult, StronglyConnectedComponentsResult, WeightDomain, execute_domain, execute_graph_theory,
};
use athena_graph::{GraphDirection, NodeId as GraphNodeId};

fn sample_graph() -> GraphObject {
    let edges =
        vec![(GraphNodeId(0), GraphNodeId(1), 1), (GraphNodeId(1), GraphNodeId(2), 1), (GraphNodeId(3), GraphNodeId(4), 1)];
    GraphObject::from_edges(
        GraphHandle { id: 1, node_count: 5 },
        GraphSemantics {
            direction: GraphDirection::Undirected,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        edges,
    )
}

#[test]
fn connected_components_via_domain_request() {
    let req = DomainRequest::GraphTheory(GraphTheoryRequest::ConnectedComponents { graph: sample_graph() });
    let DomainResult::GraphTheory(GraphTheoryResult::Exact { value }) = execute_domain(req).unwrap()
    else {
        panic!("expected exact graph_theory result");
    };
    let GraphTheoryValue::ConnectedComponents(ConnectedComponentsResult { component_count, property, .. }) = value
    else {
        panic!("expected connected components");
    };
    assert_eq!(component_count, 2);
    assert_eq!(property.state, GraphPropertyState::ProvenTrue);
}

#[test]
fn strongly_connected_components_cycle() {
    let graph = GraphObject::from_edges(
        GraphHandle { id: 10, node_count: 3 },
        GraphSemantics {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        vec![
            (GraphNodeId(0), GraphNodeId(1), 1),
            (GraphNodeId(1), GraphNodeId(2), 1),
            (GraphNodeId(2), GraphNodeId(0), 1),
        ],
    );
    let result = execute_graph_theory(GraphTheoryRequest::StronglyConnectedComponents { graph });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact");
    };
    let GraphTheoryValue::StronglyConnectedComponents(StronglyConnectedComponentsResult {
        component_count,
        property,
        ..
    }) = value
    else {
        panic!("expected scc");
    };
    assert_eq!(component_count, 1);
    assert_eq!(property.state, GraphPropertyState::ProvenTrue);
    assert!(matches!(property.certificate, Some(GraphCertificate::TraversalWitness { algorithm: "kosaraju", .. })));
}

#[test]
fn strongly_connected_components_rejects_undirected() {
    let result = execute_graph_theory(GraphTheoryRequest::StronglyConnectedComponents { graph: sample_graph() });
    assert!(matches!(result, GraphTheoryResult::Unevaluated { .. }));
}

#[test]
fn bipartite_true_with_coloring_certificate() {
    let graph = GraphObject::from_edges(
        GraphHandle { id: 11, node_count: 4 },
        GraphSemantics {
            direction: GraphDirection::Undirected,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        vec![
            (GraphNodeId(0), GraphNodeId(1), 1),
            (GraphNodeId(1), GraphNodeId(2), 1),
            (GraphNodeId(2), GraphNodeId(3), 1),
        ],
    );
    let result = execute_graph_theory(GraphTheoryRequest::Bipartite { graph });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact");
    };
    let GraphTheoryValue::Bipartite(BipartiteResult::Bipartite { property, left, right, .. }) = value
    else {
        panic!("expected bipartite");
    };
    assert_eq!(property.state, GraphPropertyState::ProvenTrue);
    assert_eq!(left.len() + right.len(), 4);
    assert!(matches!(property.certificate, Some(GraphCertificate::BipartiteColoring { .. })));
}

#[test]
fn bipartite_false_with_odd_cycle_certificate() {
    let graph = GraphObject::from_edges(
        GraphHandle { id: 12, node_count: 3 },
        GraphSemantics {
            direction: GraphDirection::Undirected,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        vec![
            (GraphNodeId(0), GraphNodeId(1), 1),
            (GraphNodeId(1), GraphNodeId(2), 1),
            (GraphNodeId(2), GraphNodeId(0), 1),
        ],
    );
    let result = execute_graph_theory(GraphTheoryRequest::Bipartite { graph });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact");
    };
    let GraphTheoryValue::Bipartite(BipartiteResult::NotBipartite { property }) = value
    else {
        panic!("expected not bipartite");
    };
    assert_eq!(property.state, GraphPropertyState::ProvenFalse);
    let Some(GraphCertificate::OddCycle { cycle, .. }) = property.certificate
    else {
        panic!("expected odd cycle certificate");
    };
    assert!(cycle.len() >= 4);
}

#[test]
fn minimum_spanning_forest_weighted() {
    let graph = GraphObject::from_edges(
        GraphHandle { id: 13, node_count: 4 },
        GraphSemantics {
            direction: GraphDirection::Undirected,
            allows_self_loops: false,
            weight_domain: WeightDomain::NonNegativeInteger,
        },
        vec![
            (GraphNodeId(0), GraphNodeId(1), 1),
            (GraphNodeId(1), GraphNodeId(2), 2),
            (GraphNodeId(0), GraphNodeId(2), 9),
            (GraphNodeId(3), GraphNodeId(3), 1),
        ],
    );
    let result = execute_graph_theory(GraphTheoryRequest::MinimumSpanningForest { graph });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact");
    };
    let GraphTheoryValue::MinimumSpanningForest(MinimumSpanningForestResult {
        total_weight,
        tree_count,
        edges,
        property,
    }) = value
    else {
        panic!("expected mst");
    };
    assert_eq!(total_weight, 3);
    assert_eq!(tree_count, 2);
    assert_eq!(edges.len(), 2);
    assert_eq!(property.state, GraphPropertyState::ProvenTrue);
    assert!(matches!(
        property.certificate,
        Some(GraphCertificate::KruskalForest { edge_count: 2, tree_count: 2, .. })
    ));
}

#[test]
fn shortest_path_unweighted() {
    let edges = vec![(GraphNodeId(0), GraphNodeId(1), 0), (GraphNodeId(1), GraphNodeId(2), 0)];
    let graph = GraphObject::from_edges(
        GraphHandle { id: 2, node_count: 3 },
        GraphSemantics {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        edges,
    );
    let result =
        execute_graph_theory(GraphTheoryRequest::ShortestPath { graph, source: GraphNodeId(0), target: GraphNodeId(2) });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact");
    };
    let GraphTheoryValue::ShortestPath(ShortestPathResult::Found { distance, path, .. }) = value
    else {
        panic!("expected found path");
    };
    assert_eq!(distance, 2);
    assert_eq!(path, vec![GraphNodeId(0), GraphNodeId(1), GraphNodeId(2)]);
}

#[test]
fn shortest_path_weighted() {
    let graph = GraphObject::from_edges(
        GraphHandle { id: 3, node_count: 3 },
        GraphSemantics {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            weight_domain: WeightDomain::NonNegativeInteger,
        },
        vec![(GraphNodeId(0), GraphNodeId(1), 5), (GraphNodeId(0), GraphNodeId(2), 1), (GraphNodeId(2), GraphNodeId(1), 1)],
    );
    let result =
        execute_graph_theory(GraphTheoryRequest::ShortestPath { graph, source: GraphNodeId(0), target: GraphNodeId(1) });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact");
    };
    let GraphTheoryValue::ShortestPath(ShortestPathResult::Found { distance, .. }) = value
    else {
        panic!("expected found");
    };
    assert_eq!(distance, 2);
}

#[test]
fn session_execute_graph_theory() {
    use athena_engine::Session;
    let session = Session::default();
    let result = session.execute_graph_theory(GraphTheoryRequest::ConnectedComponents { graph: sample_graph() });
    assert!(matches!(result, GraphTheoryResult::Exact { .. }));
}

#[test]
fn shortest_path_unreachable() {
    let graph = GraphObject::from_edges(
        GraphHandle { id: 4, node_count: 2 },
        GraphSemantics {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        vec![(GraphNodeId(0), GraphNodeId(1), 1)],
    );
    let result =
        execute_graph_theory(GraphTheoryRequest::ShortestPath { graph, source: GraphNodeId(1), target: GraphNodeId(0) });
    let GraphTheoryResult::Exact { value } = result
    else {
        panic!("expected exact unreachable");
    };
    assert!(matches!(value, GraphTheoryValue::ShortestPath(ShortestPathResult::Unreachable { .. })));
}
