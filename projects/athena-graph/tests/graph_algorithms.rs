//! In-memory graph algorithm contract tests.

use athena_graph::{
    bfs_order, connected_components, topological_sort, Graph, GraphDirection, NodeId,
};

#[test]
fn bfs_visits_reachable_nodes() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(b, c, ());
    let order = bfs_order(&g, a).unwrap();
    assert_eq!(order, vec![a, b, c]);
}

#[test]
fn topo_sort_detects_cycle() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(b, a, ());
    assert!(topological_sort(&g).is_err());
}

#[test]
fn topo_sort_orders_dag() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(a, c, ());
    g.add_edge(b, c, ());
    let order = topological_sort(&g).unwrap();
    assert_eq!(order[0], a);
    assert_eq!(*order.last().unwrap(), c);
}

#[test]
fn undirected_topo_rejected() {
    let g = Graph::<(), ()>::new(GraphDirection::Undirected);
    assert!(matches!(
        topological_sort(&g),
        Err(athena_graph::GraphError::UndirectedTopo)
    ));
}

#[test]
fn connected_components_labels_undirected() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Undirected);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    let labels = connected_components(&g);
    assert_eq!(labels[a.0 as usize], a);
    assert_eq!(labels[b.0 as usize], a);
    assert_eq!(labels[c.0 as usize], c);
}
