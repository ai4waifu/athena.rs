//! 内存图算法：BFS、拓扑排序、连通分量。

use crate::{Graph, GraphDirection, GraphError, NodeId};

/// 确定性 BFS 访问顺序（有向图沿出边；无向图沿邻接）。
pub fn bfs_order<N, E>(graph: &Graph<N, E>, start: NodeId) -> Result<Vec<NodeId>, GraphError> {
    if start.0 >= graph.node_count() {
        return Err(GraphError::InvalidNode);
    }
    let mut seen = vec![false; graph.node_count() as usize];
    let mut order = Vec::new();
    let mut queue = std::collections::VecDeque::from([start]);
    seen[start.0 as usize] = true;
    while let Some(node) = queue.pop_front() {
        order.push(node);
        for next in graph.neighbors(node) {
            let i = next.0 as usize;
            if !seen[i] {
                seen[i] = true;
                queue.push_back(next);
            }
        }
    }
    Ok(order)
}

/// DAG 拓扑排序；存在环时返回 [`GraphError::CycleDetected`]。
pub fn topological_sort<N, E>(graph: &Graph<N, E>) -> Result<Vec<NodeId>, GraphError> {
    if graph.direction() != GraphDirection::Directed {
        return Err(GraphError::UndirectedTopo);
    }
    let n = graph.node_count() as usize;
    let mut indegree = vec![0usize; n];
    for node in 0..n {
        for target in graph.neighbors(NodeId(node as u64)) {
            indegree[target.0 as usize] += 1;
        }
    }
    let mut ready: std::collections::VecDeque<NodeId> = (0..n)
        .filter(|&i| indegree[i] == 0)
        .map(|i| NodeId(i as u64))
        .collect();
    let mut order = Vec::with_capacity(n);
    while let Some(node) = ready.pop_front() {
        order.push(node);
        for target in graph.neighbors(node) {
            let i = target.0 as usize;
            indegree[i] -= 1;
            if indegree[i] == 0 {
                ready.push_back(target);
            }
        }
    }
    if order.len() != n {
        Err(GraphError::CycleDetected)
    } else {
        Ok(order)
    }
}

/// 弱连通分量（有向图按底层无向邻接解释；无向图按邻接）。
///
/// 返回每个节点的分量标签，标签为该分量中最小 `NodeId`。
pub fn connected_components<N, E>(graph: &Graph<N, E>) -> Vec<NodeId> {
    let n = graph.node_count() as usize;
    let mut labels = (0..n).map(|i| NodeId(i as u64)).collect::<Vec<_>>();
    let mut adj = vec![Vec::new(); n];
    for u in 0..n {
        let uid = NodeId(u as u64);
        for v in graph.neighbors(uid) {
            adj[u].push(v);
            if graph.direction() == GraphDirection::Directed {
                adj[v.0 as usize].push(uid);
            }
        }
    }
    let mut seen = vec![false; n];
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let root = NodeId(start as u64);
        let mut stack = vec![root];
        seen[start] = true;
        labels[start] = root;
        while let Some(node) = stack.pop() {
            for next in &adj[node.0 as usize] {
                let i = next.0 as usize;
                if !seen[i] {
                    seen[i] = true;
                    labels[i] = root;
                    stack.push(*next);
                }
            }
        }
    }
    labels
}
