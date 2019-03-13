//! 图存储转换、CSC、SCC、视图与 capability 合同测试。

use athena_graph::{
    EdgeFilteredView, Graph, GraphAlgorithmRequirements, GraphDirection, GraphRevision, InducedSubgraphView, ReversedGraphView,
    csr_to_csc, edge_list_to_csr, graph_to_csr,
};
use athena_graph::primitives::{UnionFind, connected_components, strongly_connected_components};
use athena_ndarray::MemoryBudget;

#[test]
fn revision_increments_on_mutation() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    assert_eq!(g.revision(), GraphRevision(0));
    let a = g.add_node(());
    assert_eq!(g.revision(), GraphRevision(1));
    let b = g.add_node(());
    g.add_edge(a, b, ());
    assert_eq!(g.revision(), GraphRevision(3));
}

#[test]
fn strongly_connected_components_labels_cycle() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(b, c, ());
    g.add_edge(c, a, ());
    g.add_edge(c, b, ());
    let labels = strongly_connected_components(&g).unwrap();
    assert_eq!(labels[a.0 as usize], a);
    assert_eq!(labels[b.0 as usize], a);
    assert_eq!(labels[c.0 as usize], a);
}

#[test]
fn scc_rejects_undirected() {
    let g = Graph::<(), ()>::new(GraphDirection::Undirected);
    assert!(strongly_connected_components(&g).is_err());
}

#[test]
fn graph_to_csr_and_csc_roundtrip_neighbors() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(a, c, ());
    g.add_edge(c, b, ());
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = graph_to_csr(&g, budget).unwrap();
    assert_eq!(csr.node_count(), 3);
    assert_eq!(csr.edge_count(), 3);
    let mut out_b = Vec::new();
    csr.for_each_neighbor_chunk(a.0, |chunk| out_b.extend_from_slice(chunk)).unwrap();
    assert_eq!(out_b, vec![b.0, c.0]);
    let csc = csr_to_csc(&csr, budget).unwrap();
    let mut in_b = Vec::new();
    csc.for_each_in_neighbor_chunk(b.0, |chunk| in_b.extend_from_slice(chunk)).unwrap();
    in_b.sort_unstable();
    assert_eq!(in_b, vec![a.0, c.0]);
}

#[test]
fn edge_list_to_csr_sorted() {
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(3, vec![(1, 2), (0, 1), (0, 2)], budget).unwrap();
    let mut n0 = Vec::new();
    csr.for_each_neighbor_chunk(0, |c| n0.extend_from_slice(c)).unwrap();
    assert_eq!(n0, vec![1, 2]);
}

#[test]
fn reversed_view_swaps_direction() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    g.add_edge(a, b, ());
    let rev = ReversedGraphView::new(&g).unwrap();
    let preds: Vec<_> = rev.neighbors(b).unwrap().collect();
    assert_eq!(preds, vec![a]);
}

#[test]
fn induced_subgraph_keeps_internal_edges_only() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(b, c, ());
    let sub = InducedSubgraphView::new(&g, [a, b]);
    assert_eq!(sub.node_count().unwrap(), 2);
    let n: Vec<_> = sub.neighbors(a).unwrap().collect();
    assert_eq!(n, vec![b]);
    assert!(sub.neighbors(b).unwrap().next().is_none());
}

#[test]
fn edge_filtered_view_drops_edges() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(a, c, ());
    let view = EdgeFilteredView::from_view(g.view(), |_, target, _| target.0 == b.0);
    let n: Vec<_> = view.neighbors(a).unwrap().collect();
    assert_eq!(n, vec![b]);
}

#[test]
fn capabilities_match_requirements() {
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
    let caps = g.capabilities();
    assert!(caps.satisfies(GraphAlgorithmRequirements::in_memory_traversal()));
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    let csr_caps = csr.capabilities();
    assert!(csr_caps.satisfies(GraphAlgorithmRequirements::chunked_csr_scan()));
    let csc = csr_to_csc(&csr, budget).unwrap();
    let req = GraphAlgorithmRequirements { reverse_adjacency: true, ..GraphAlgorithmRequirements::chunked_csr_scan() };
    assert!(csc.capabilities().satisfies(req));
}

#[test]
fn union_find_merges_components() {
    let mut uf = UnionFind::new(4);
    uf.union(0, 1);
    uf.union(2, 3);
    assert_eq!(uf.set_count(), 2);
    uf.union(1, 2);
    assert_eq!(uf.set_count(), 1);
}

#[test]
fn connected_components_unchanged_for_undirected() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Undirected);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    let labels = connected_components(&g);
    assert_eq!(labels[c.0 as usize], c);
}
