//! 图身份 / snapshot / view mapping / CSR 单调校验合同。

use athena_graph::{
    CsrGraph, Graph, GraphBuilder, GraphDirection, GraphId, GraphRevision, GraphSemantics, InducedSubgraphView,
    ReversedGraphView, ViewTransform,
};
use athena_ndarray::{ChunkedArray, InMemoryStorage, LogicalShape, MemoryBudget};

#[test]
fn distinct_graphs_share_revision_zero_but_not_id() {
    let a = Graph::<(), ()>::new(GraphDirection::Directed);
    let b = Graph::<(), ()>::new(GraphDirection::Directed);
    assert_eq!(a.revision(), GraphRevision(0));
    assert_eq!(b.revision(), GraphRevision(0));
    assert_ne!(a.id(), b.id());
}

#[test]
fn transaction_bumps_revision_once() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    g.transaction(|g| {
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
    });
    assert_eq!(g.revision(), GraphRevision(1));
}

#[test]
fn builder_finish_yields_immutable_snapshot() {
    let mut builder = GraphBuilder::with_id(GraphId::from_raw(42), GraphSemantics::directed_simple());
    let a = builder.add_node(());
    let b = builder.add_node(());
    builder.add_edge(a, b, ());
    let frozen = builder.finish();
    assert_eq!(frozen.id(), GraphId::from_raw(42));
    let snap = frozen.snapshot();
    assert_eq!(snap.graph_id, GraphId::from_raw(42));
    assert_eq!(snap.revision, frozen.revision());
    assert_eq!(frozen.graph().node_ref(a).graph_id, GraphId::from_raw(42));
}

#[test]
fn node_ref_carries_graph_and_revision() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let n = g.add_node(());
    let r = g.node_ref(n);
    assert_eq!(r.graph_id, g.id());
    assert_eq!(r.revision, g.revision());
    assert_eq!(r.node, n);
    assert_eq!(g.resolve_node_ref(r).unwrap(), n);
}

#[test]
fn node_ref_stale_after_mutation() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let n = g.add_node(());
    let stale = g.node_ref(n);
    let _ = g.add_node(());
    assert!(matches!(
        g.resolve_node_ref(stale),
        Err(athena_graph::GraphError::StaleRef { .. })
    ));
}

#[test]
fn self_loop_rejected_when_disallowed() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    assert!(g.add_edge(a, a, ()).is_none());
}

#[test]
fn reversed_view_mapping_is_identity_on_base_ids() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let e = g.add_edge(a, b, ()).unwrap();
    let rev = ReversedGraphView::new(&g).unwrap();
    assert_eq!(rev.mapping().transform, ViewTransform::Reversed);
    assert_eq!(rev.base_graph_id(), g.id());
    assert_eq!(rev.base_revision(), g.revision());
    assert_eq!(rev.map_node_to_base(a).unwrap(), Some(a));
    assert_eq!(rev.map_edge_to_base(e).unwrap(), Some(e));
    let vn = rev.view_node_ref(a).unwrap();
    assert_eq!(rev.map_view_node_to_source(vn).unwrap().node, a);
}

#[test]
fn view_stale_after_base_mutation() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    g.add_edge(a, b, ());
    let rev = ReversedGraphView::new(&g).unwrap();
    let mapping = rev.mapping().clone();
    let vn = rev.view_node_ref(a).unwrap();
    let expected_rev = mapping.base_revision;
    drop(rev);
    let _ = g.add_node(());
    assert!(matches!(
        mapping.ensure_fresh(g.id(), g.revision()),
        Err(athena_graph::GraphError::StaleView { expected, actual }) if expected == expected_rev && actual == g.revision()
    ));
    let rev2 = ReversedGraphView::new(&g).unwrap();
    assert!(rev2.ensure_fresh().is_ok());
    assert!(matches!(rev2.map_view_node_to_source(vn), Err(athena_graph::GraphError::WrongView { .. })));
}

#[test]
fn induced_view_maps_only_kept_nodes_and_internal_edges() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    let e_ab = g.add_edge(a, b, ()).unwrap();
    let e_bc = g.add_edge(b, c, ()).unwrap();
    let induced = InducedSubgraphView::new(&g, [a, b]);
    assert_eq!(induced.map_node_to_base(a).unwrap(), Some(a));
    assert_eq!(induced.map_node_to_base(c).unwrap(), None);
    assert_eq!(induced.map_edge_to_base(e_ab).unwrap(), Some(e_ab));
    assert_eq!(induced.map_edge_to_base(e_bc).unwrap(), None);
    assert_eq!(induced.mapping().transform, ViewTransform::Induced);
    let vn = induced.view_node_ref(a).unwrap();
    assert_eq!(induced.map_view_node_to_source(vn).unwrap().node, a);
    assert!(induced.view_node_ref(c).is_err());
}

#[test]
fn csr_rejects_non_monotonic_offsets() {
    let budget = MemoryBudget::new(4096).unwrap();
    // [0, 5, 3, 10]：首尾若单独看可能“碰巧”，但中间非单调。
    let offsets =
        ChunkedArray::new(LogicalShape::new([4]).unwrap(), InMemoryStorage::from_vec(vec![0, 5, 3, 10]), budget).unwrap();
    let indices = ChunkedArray::new(LogicalShape::new([10]).unwrap(), InMemoryStorage::from_vec(vec![0; 10]), budget).unwrap();
    let err = CsrGraph::new(3, offsets, indices).unwrap_err();
    assert!(matches!(err, athena_graph::GraphError::OffsetNonMonotonic { .. }));
}

#[test]
fn graph_to_csr_binds_storage_metadata() {
    use athena_graph::graph_to_csr;
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    g.add_edge(a, b, ());
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = graph_to_csr(&g, budget).unwrap();
    let meta = csr.metadata().expect("metadata");
    assert_eq!(meta.graph_id, Some(g.id()));
    assert_eq!(meta.revision, Some(g.revision()));
}
