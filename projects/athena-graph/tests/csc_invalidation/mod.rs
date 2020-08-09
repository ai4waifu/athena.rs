//! 派生 CSC 相对源 CSR revision 失效。

use athena_graph::{DerivedCsc, GraphBuilder, GraphDirection, GraphError, graph_to_csr};
use athena_ndarray::MemoryBudget;

#[test]
fn derived_csc_valid_for_same_csr_revision() {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let a = b.add_node(());
    let bb = b.add_node(());
    b.add_edge(a, bb, ());
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = graph_to_csr(b.graph(), budget).unwrap();
    let derived = DerivedCsc::from_csr(&csr, budget).unwrap();
    assert!(derived.is_valid_for(&csr));
    let csc = derived.ensure_valid_for(&csr).unwrap();
    assert_eq!(csc.node_count(), 2);
    assert_eq!(derived.source_revision(), csr.metadata().and_then(|m| m.revision));
    assert_eq!(derived.source_graph_id(), Some(b.graph().id()));
}

#[test]
fn derived_csc_stale_after_graph_mutation_rebuilds_csr() {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let a = b.add_node(());
    let bb = b.add_node(());
    b.add_edge(a, bb, ());
    let budget = MemoryBudget::new(4096).unwrap();
    let csr_v1 = graph_to_csr(b.graph(), budget).unwrap();
    let derived = DerivedCsc::from_csr(&csr_v1, budget).unwrap();
    let old_rev = derived.source_revision().unwrap();

    let c = b.add_node(());
    b.add_edge(bb, c, ());
    let csr_v2 = graph_to_csr(b.graph(), budget).unwrap();
    assert_ne!(csr_v2.metadata().and_then(|m| m.revision), Some(old_rev));
    assert!(!derived.is_valid_for(&csr_v2));
    match derived.ensure_valid_for(&csr_v2) {
        Err(GraphError::StaleCsc { derived_from, current }) => {
            assert_eq!(derived_from, old_rev);
            assert_eq!(current, csr_v2.metadata().and_then(|m| m.revision));
        }
        other => panic!("expected StaleCsc, got {other:?}"),
    }
}

#[test]
fn csr_to_csc_inherits_snapshot_metadata() {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    let a = b.add_node(());
    let bb = b.add_node(());
    b.add_edge(a, bb, ());
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = graph_to_csr(b.graph(), budget).unwrap();
    let csc = athena_graph::csr_to_csc(&csr, budget).unwrap();
    let meta = csc.metadata().expect("csc metadata");
    assert_eq!(meta.graph_id, Some(b.graph().id()));
    assert_eq!(meta.revision, Some(b.graph().revision()));
    assert_eq!(meta.representation_id, athena_graph::RepresentationId::CSC);
}
