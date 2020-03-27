//! 图身份 / snapshot / view mapping / CSR 单调校验合同。

use athena_graph::{
    CsrGraph, GraphBuilder, GraphDirection, GraphId, GraphRevision, GraphSemantics, ImmutableGraph, InducedSubgraphView, MutableGraph,
    ReversedGraphView, ViewTransform, graph_to_csr,
};
use athena_ndarray::{ChunkedArray, InMemoryStorage, LogicalShape, MemoryBudget};

fn build(direction: GraphDirection, f: impl FnOnce(&mut MutableGraph<(), ()>)) -> ImmutableGraph<(), ()> {
    let mut b = GraphBuilder::<(), ()>::from_direction(direction);
    f(b.graph_mut());
    b.finish()
}

#[test]
fn distinct_graphs_share_revision_zero_but_not_id() {
    let a = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    let b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    assert_eq!(a.revision(), GraphRevision(0));
    assert_eq!(b.revision(), GraphRevision(0));
    assert_ne!(a.id(), b.id());
}

#[test]
fn transaction_bumps_revision_once() {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    b.transaction(|g| {
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
    });
    let g = b.finish();
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
    assert_eq!(frozen.node_ref(a).graph_id, GraphId::from_raw(42));
}

#[test]
fn node_ref_carries_graph_and_revision() {
    let g = build(GraphDirection::Directed, |g| {
        let _ = g.add_node(());
    });
    let n = athena_graph::NodeId(0);
    let r = g.node_ref(n);
    assert_eq!(r.graph_id, g.id());
    assert_eq!(r.revision, g.revision());
    assert_eq!(r.node, n);
    assert_eq!(g.resolve_node_ref(r).unwrap(), n);
}

#[test]
fn node_ref_stale_after_mutation() {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let n = b.add_node(());
    let stale = b.graph().node_ref(n);
    let _ = b.add_node(());
    let g = b.finish();
    assert!(matches!(g.resolve_node_ref(stale), Err(athena_graph::GraphError::StaleRef { .. })));
}

#[test]
fn self_loop_rejected_when_disallowed() {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let a = b.add_node(());
    assert!(b.add_edge(a, a, ()).is_none());
}

#[test]
fn reversed_view_mapping_is_identity_on_base_ids() {
    let g = build(GraphDirection::Directed, |g| {
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
    });
    let a = athena_graph::NodeId(0);
    let e = athena_graph::EdgeId(0);
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
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let a = b.add_node(());
    let bb = b.add_node(());
    b.add_edge(a, bb, ());
    let mapping;
    let vn;
    let expected_rev;
    {
        let rev = ReversedGraphView::new(b.graph()).unwrap();
        mapping = *rev.mapping();
        vn = rev.view_node_ref(a).unwrap();
        expected_rev = mapping.base_revision;
    }
    let _ = b.add_node(());
    let g = b.finish();
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
    let g = build(GraphDirection::Directed, |g| {
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
    });
    let a = athena_graph::NodeId(0);
    let b = athena_graph::NodeId(1);
    let c = athena_graph::NodeId(2);
    let e_ab = athena_graph::EdgeId(0);
    let e_bc = athena_graph::EdgeId(1);
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
    let offsets = ChunkedArray::new(LogicalShape::new([4]).unwrap(), InMemoryStorage::from_vec(vec![0, 5, 3, 10]), budget).unwrap();
    let indices = ChunkedArray::new(LogicalShape::new([10]).unwrap(), InMemoryStorage::from_vec(vec![0; 10]), budget).unwrap();
    let err = CsrGraph::new(3, offsets, indices).unwrap_err();
    assert!(matches!(err, athena_graph::GraphError::OffsetNonMonotonic { .. }));
}

#[test]
fn graph_to_csr_binds_storage_metadata() {
    let g = build(GraphDirection::Directed, |g| {
        let a = g.add_node(());
        let b = g.add_node(());
        g.add_edge(a, b, ());
    });
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = graph_to_csr(&g, budget).unwrap();
    let meta = csr.metadata().expect("metadata");
    assert_eq!(meta.graph_id, Some(g.id()));
    assert_eq!(meta.revision, Some(g.revision()));
}

#[test]
fn payload_aware_edge_iterators_use_adj_indexes() {
    let mut b = GraphBuilder::<u8, i32>::from_direction(GraphDirection::Directed);
    let a = b.add_node(1);
    let c = b.add_node(2);
    let d = b.add_node(3);
    let e_ac = b.add_edge(a, c, 10).unwrap();
    let e_da = b.add_edge(d, a, 20).unwrap();
    let g = b.finish();

    assert_eq!(g.node_value(a), Some(&1));
    assert_eq!(g.edge_value(e_ac), Some(&10));

    let outs: Vec<_> = g.out_edges(a).collect();
    assert_eq!(outs, vec![(e_ac, c, &10)]);
    let ins: Vec<_> = g.in_edges(a).collect();
    assert_eq!(ins, vec![(e_da, d, &20)]);
}
