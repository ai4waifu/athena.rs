//! 图论与 `DomainRequest::GraphTheory` 集成测试。

use athena_engine::{
    ConnectedComponentsResult, DomainRequest, DomainResult, GraphHandle, GraphObject, GraphSemantics, GraphTheoryRequest,
    GraphTheoryResult, GraphTheoryValue, ShortestPathResult, WeightDomain, execute_domain, execute_graph_theory,
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
    let GraphTheoryValue::ConnectedComponents(ConnectedComponentsResult { component_count, .. }) = value
    else {
        panic!("expected connected components");
    };
    assert_eq!(component_count, 2);
}

#[test]
fn shortest_path_unweighted() {
    let mut edges = vec![(GraphNodeId(0), GraphNodeId(1), 0), (GraphNodeId(1), GraphNodeId(2), 0)];
    let graph = GraphObject::from_edges(
        GraphHandle { id: 2, node_count: 3 },
        GraphSemantics {
            direction: GraphDirection::Directed,
            allows_self_loops: false,
            weight_domain: WeightDomain::Unweighted,
        },
        edges.drain(..).collect(),
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
