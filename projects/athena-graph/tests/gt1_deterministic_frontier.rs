//! 确定性 frontier、稳定邻接扩展序、取消与恢复。

use athena_graph::{
    CancelFlag, DeterministicBfsOutcome, GraphBuilder, GraphDirection, ImmutableGraph, MutableGraph, NodeId, bfs_order, deterministic_bfs,
    resume_deterministic_bfs,
};

fn build(f: impl FnOnce(&mut MutableGraph<(), ()>)) -> ImmutableGraph<(), ()> {
    let mut b = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed);
    f(b.graph_mut());
    b.finish()
}

#[test]
fn bfs_order_independent_of_edge_insertion_order() {
    let g1 = build(|g| {
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, c, ());
        g.add_edge(a, b, ());
    });
    let g2 = build(|g| {
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
    });
    let o1 = bfs_order(&g1, NodeId(0)).unwrap();
    let o2 = bfs_order(&g2, NodeId(0)).unwrap();
    assert_eq!(o1.iter().map(|n| n.0).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(o2.iter().map(|n| n.0).collect::<Vec<_>>(), vec![0, 1, 2]);
}

#[test]
fn deterministic_bfs_repeats_same_order() {
    let g = build(|g| {
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        let d = g.add_node(());
        g.add_edge(a, d, ());
        g.add_edge(a, b, ());
        g.add_edge(a, c, ());
        g.add_edge(b, d, ());
    });
    let a = NodeId(0);
    let first = match deterministic_bfs(&g, a, None).unwrap() {
        DeterministicBfsOutcome::Complete(o) => o,
        other => panic!("expected complete, got {other:?}"),
    };
    let second = match deterministic_bfs(&g, a, None).unwrap() {
        DeterministicBfsOutcome::Complete(o) => o,
        other => panic!("expected complete, got {other:?}"),
    };
    assert_eq!(first, second);
    assert_eq!(first, vec![NodeId(0), NodeId(1), NodeId(2), NodeId(3)]);
}

#[test]
fn cancel_then_resume_yields_full_deterministic_order() {
    let g = build(|g| {
        let a = g.add_node(());
        let b = g.add_node(());
        let c = g.add_node(());
        g.add_edge(a, b, ());
        g.add_edge(b, c, ());
    });
    let a = NodeId(0);
    let mut flag = CancelFlag::new();
    flag.cancel();
    let checkpoint = match deterministic_bfs(&g, a, Some(&flag)).unwrap() {
        DeterministicBfsOutcome::Cancelled { partial, checkpoint } => {
            assert!(partial.is_empty());
            checkpoint
        }
        other => panic!("expected cancelled, got {other:?}"),
    };

    let resumed = match resume_deterministic_bfs(&g, checkpoint, None).unwrap() {
        DeterministicBfsOutcome::Complete(o) => o,
        other => panic!("expected complete, got {other:?}"),
    };
    assert_eq!(resumed, bfs_order(&g, a).unwrap());
    assert_eq!(resumed, vec![NodeId(0), NodeId(1), NodeId(2)]);
}
