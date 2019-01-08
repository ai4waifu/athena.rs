//! GT1j：capability 矩阵正反测（失败必须结构化，禁止偷偷物化）。

use athena_graph::{
    Graph, GraphAlgorithmRequirements, GraphDirection, GraphError, edge_list_to_csr,
};
use athena_ndarray::MemoryBudget;

#[test]
fn in_memory_graph_satisfies_in_memory_only() {
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
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
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
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
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
    assert!(matches!(
        g.ensure_capabilities(GraphAlgorithmRequirements::external_workspace()),
        Err(GraphError::CapabilityMismatch { .. })
    ));
}

#[test]
fn memory_graph_satisfies_random_access_rejects_when_disabled() {
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
    g.ensure_capabilities(GraphAlgorithmRequirements::random_access_storage()).unwrap();
    // 构造一个无随机访问的假 capability 场景：CSR 有 random_access，内存图也有。
    // 反例：要求 sorted_adjacency 时内存图失败。
    let req = GraphAlgorithmRequirements {
        sorted_adjacency: true,
        ..GraphAlgorithmRequirements::in_memory_traversal()
    };
    assert!(matches!(g.ensure_capabilities(req), Err(GraphError::CapabilityMismatch { .. })));
}

#[test]
fn multi_pass_rejected_without_in_memory_or_chunked() {
    // 裸 requirements：既非 in_memory 也非 chunked → multi_pass 失败。
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
    // CSR 有 chunked_sequential，故 multi_pass 可通过。
    csr.ensure_capabilities(GraphAlgorithmRequirements::multi_pass()).unwrap();
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
    // 内存图 in_memory=true，也可 multi_pass。
    g.ensure_capabilities(GraphAlgorithmRequirements::multi_pass()).unwrap();
}

#[test]
fn distributed_shards_rejected_on_all_current_backends() {
    let g = Graph::<(), ()>::new(GraphDirection::Directed);
    assert!(matches!(
        g.ensure_capabilities(GraphAlgorithmRequirements::distributed_shards()),
        Err(GraphError::CapabilityMismatch { .. })
    ));
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    assert!(matches!(
        csr.ensure_capabilities(GraphAlgorithmRequirements::distributed_shards()),
        Err(GraphError::CapabilityMismatch { .. })
    ));
}

#[test]
fn csr_rejects_reverse_adjacency_csc_accepts() {
    let budget = MemoryBudget::new(4096).unwrap();
    let csr = edge_list_to_csr(2, vec![(0, 1)], budget).unwrap();
    let req = GraphAlgorithmRequirements {
        reverse_adjacency: true,
        ..GraphAlgorithmRequirements::chunked_csr_scan()
    };
    assert!(matches!(csr.ensure_capabilities(req), Err(GraphError::CapabilityMismatch { .. })));
    let csc = athena_graph::csr_to_csc(&csr, budget).unwrap();
    csc.ensure_capabilities(req).unwrap();
}
