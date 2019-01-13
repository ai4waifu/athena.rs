//! 确定性 frontier、稳定邻接扩展序、取消与恢复。

use athena_graph::{
    CancelFlag, DeterministicBfsOutcome, Graph, GraphDirection, bfs_order, deterministic_bfs, resume_deterministic_bfs,
};

#[test]
fn bfs_order_independent_of_edge_insertion_order() {
    let mut g1 = Graph::<(), ()>::new(GraphDirection::Directed);
    let a1 = g1.add_node(());
    let b1 = g1.add_node(());
    let c1 = g1.add_node(());
    // 先连大 NodeId，再连小 NodeId
    g1.add_edge(a1, c1, ());
    g1.add_edge(a1, b1, ());

    let mut g2 = Graph::<(), ()>::new(GraphDirection::Directed);
    let a2 = g2.add_node(());
    let b2 = g2.add_node(());
    let c2 = g2.add_node(());
    g2.add_edge(a2, b2, ());
    g2.add_edge(a2, c2, ());

    let o1 = bfs_order(&g1, a1).unwrap();
    let o2 = bfs_order(&g2, a2).unwrap();
    // 相对局部编号应同构：start, then smaller neighbor, then larger
    assert_eq!(o1.iter().map(|n| n.0).collect::<Vec<_>>(), vec![0, 1, 2]);
    assert_eq!(o2.iter().map(|n| n.0).collect::<Vec<_>>(), vec![0, 1, 2]);
}

#[test]
fn deterministic_bfs_repeats_same_order() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    let d = g.add_node(());
    g.add_edge(a, d, ());
    g.add_edge(a, b, ());
    g.add_edge(a, c, ());
    g.add_edge(b, d, ());
    let first = match deterministic_bfs(&g, a, None).unwrap() {
        DeterministicBfsOutcome::Complete(o) => o,
        other => panic!("expected complete, got {other:?}"),
    };
    let second = match deterministic_bfs(&g, a, None).unwrap() {
        DeterministicBfsOutcome::Complete(o) => o,
        other => panic!("expected complete, got {other:?}"),
    };
    assert_eq!(first, second);
    assert_eq!(first, vec![a, b, c, d]);
}

#[test]
fn cancel_then_resume_yields_full_deterministic_order() {
    let mut g = Graph::<(), ()>::new(GraphDirection::Directed);
    let a = g.add_node(());
    let b = g.add_node(());
    let c = g.add_node(());
    g.add_edge(a, b, ());
    g.add_edge(b, c, ());

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
    assert_eq!(resumed, vec![a, b, c]);
}
