//! 自 `src/domains/views/graph_matrix.rs` 迁出的原内联测试。

use athena_engine::domains::{
    graph_theory::{GraphDomainSemantics, GraphHandle, GraphObject, WeightDomain},
    views::{GraphMatrixView, ViewKind},
};
use athena_graph::{GraphDirection, NodeId};

#[test]
fn graph_matrix_view_borrows_edges() {
    let handle = GraphHandle { id: 1, node_count: 2 };
    let graph = GraphObject::from_edges(
        handle,
        GraphDomainSemantics::new(GraphDirection::Directed, WeightDomain::Unweighted),
        vec![(NodeId(0), NodeId(1), 1)],
    );
    let view = GraphMatrixView::open(&graph);
    assert_eq!(view.header().kind, ViewKind::GraphMatrix);
    assert_eq!(view.node_count(), 2);
    assert_eq!(view.nnz(), 1);
    assert_eq!(view.edges()[0].0, NodeId(0));
}
