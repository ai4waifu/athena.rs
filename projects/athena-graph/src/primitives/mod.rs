//! L0 结构原语：确定性 frontier、遍历与分量扫描。
//!
//! 分量 / 拓扑标签是扫描原语，**不是**已证图论 claim。领域结论经 `athena-engine::graph_theory` 包装。

mod components;
mod frontier;
mod traversal;
mod union_find;

pub use components::{connected_components, strongly_connected_components, topological_sort};
pub use frontier::{
    CancelFlag, DeterministicBfsOutcome, DeterministicFrontier, FrontierCheckpoint, deterministic_bfs, resume_deterministic_bfs,
    sort_neighbors_deterministic,
};
pub use traversal::bfs_order;
pub use union_find::UnionFind;
