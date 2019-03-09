//! 遍历原语：确定性 BFS 访问序。

use crate::{DeterministicBfsOutcome, Graph, GraphError, NodeId, deterministic_bfs};

/// 确定性 BFS 访问顺序（有向图沿出边；无向图沿邻接）。
///
/// 扩展序：邻接目标按 [`NodeId`] 升序入队（与边插入顺序无关）。完整可取消/可恢复 API 见
/// [`crate::primitives::deterministic_bfs`]。
pub fn bfs_order<N, E>(graph: &Graph<N, E>, start: NodeId) -> Result<Vec<NodeId>, GraphError> {
    match deterministic_bfs(graph, start, None)? {
        DeterministicBfsOutcome::Complete(order) => Ok(order),
        DeterministicBfsOutcome::Cancelled { .. } => unreachable!("cancel is None"),
    }
}
