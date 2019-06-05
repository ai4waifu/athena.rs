//! capability 矩阵正反测（失败必须结构化，禁止偷偷物化）。

use athena_graph::{GraphAlgorithmRequirements, GraphBuilder, GraphDirection, GraphError, edge_list_to_csr};
use athena_ndarray::MemoryBudget;

#[test]
fn in_memory_graph_satisfies_in_memory_only() {
    let g = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    g.ensure_capabilities(GraphAlgorithmRequirements::in_memory_only()).unwrap();
}

#[test]
fn csr_rejects_in_memory_only() {
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    let err = csr.ensure_capabilities(GraphAlgorithmRequirements::in_memory_only()).unwrap_err();
    assert!(matches!(err, GraphError::CapabilityMismatch { .. }));
    assert!(!csr.capabilities().satisfies(GraphAlgorithmRequirements::in_memory_only()));
}

#[test]
fn memory_graph_rejects_chunked_sequential() {
    let g = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    let err = g.ensure_capabilities(GraphAlgorithmRequirements::chunked_sequential()).unwrap_err();
    assert!(matches!(err, GraphError::CapabilityMismatch { .. }));
}

#[test]
fn csr_satisfies_chunked_sequential_and_external_workspace() {
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    csr.ensure_capabilities(GraphAlgorithmRequirements::chunked_sequential()).unwrap();
    csr.ensure_capabilities(GraphAlgorithmRequirements::external_workspace()).unwrap();
    csr.ensure_capabilities(GraphAlgorithmRequirements::random_access_storage()).unwrap();
    csr.ensure_capabilities(GraphAlgorithmRequirements::chunked_csr_scan()).unwrap();
}

#[test]
fn memory_graph_rejects_external_workspace() {
    let g = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    assert!(matches!(g.ensure_capabilities(GraphAlgorithmRequirements::external_workspace()), Err(GraphError::CapabilityMismatch { .. })));
}

#[test]
fn memory_graph_satisfies_random_access_rejects_when_disabled() {
    let g = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    g.ensure_capabilities(GraphAlgorithmRequirements::random_access_storage()).unwrap();
    let req = GraphAlgorithmRequirements { sorted_adjacency: true, ..GraphAlgorithmRequirements::in_memory_traversal() };
    assert!(matches!(g.ensure_capabilities(req), Err(GraphError::CapabilityMismatch { .. })));
}

#[test]
fn multi_pass_rejected_without_in_memory_or_chunked() {
    let caps = athena_graph::GraphCapabilities {
        in_memory: false,
        sorted_adjacency: false,
        reverse_adjacency: false,
        random_access: false,
        chunked_sequential: false,
        external_workspace: false,
        distributed_shards: false,
    };
    assert!(!caps.satisfies(GraphAlgorithmRequirements::multi_pass()));
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    csr.ensure_capabilities(GraphAlgorithmRequirements::multi_pass()).unwrap();
    let g = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    g.ensure_capabilities(GraphAlgorithmRequirements::multi_pass()).unwrap();
}

#[test]
fn distributed_shards_rejected_on_all_current_backends() {
    let g = GraphBuilder::<(), ()>::from_direction(GraphDirection::Directed).finish();
    assert!(matches!(g.ensure_capabilities(GraphAlgorithmRequirements::distributed_shards()), Err(GraphError::CapabilityMismatch { .. })));
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    assert!(matches!(csr.ensure_capabilities(GraphAlgorithmRequirements::distributed_shards()), Err(GraphError::CapabilityMismatch { .. })));
}

#[test]
fn csr_rejects_reverse_adjacency_csc_accepts() {
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    let req = GraphAlgorithmRequirements { reverse_adjacency: true, ..GraphAlgorithmRequirements::chunked_csr_scan() };
    assert!(matches!(csr.ensure_capabilities(req), Err(GraphError::CapabilityMismatch { .. })));
    let csc = athena_graph::csr_to_csc(&csr, budget).unwrap();
    csc.ensure_capabilities(req).unwrap();
}
