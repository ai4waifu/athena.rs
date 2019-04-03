//! 分量扫描原语（供 `graph_theory` 包装证书；不是已证领域 API）。

use std::collections::VecDeque;

use crate::{GraphDirection, GraphError, MutableGraph, NodeId};

/// DAG 拓扑排序原语；存在环时返回 [`GraphError::CycleDetected`]。
///
/// 领域完成态须经 `athena-engine::graph_theory` 的 `GraphPropertyState` 包装。
pub fn topological_sort<N, E>(graph: &MutableGraph<N, E>) -> Result<Vec<NodeId>, GraphError> {
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
    let mut ready: VecDeque<NodeId> = (0..n).filter(|&i| indegree[i] == 0).map(|i| NodeId(i as u64)).collect();
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
    if order.len() != n { Err(GraphError::CycleDetected) } else { Ok(order) }
}

/// 弱连通分量标签扫描（有向图按底层无向邻接解释）。
///
/// 返回每个节点的分量标签，标签为该分量中最小 `NodeId`。领域结论须经 engine 包装。
pub fn connected_components<N, E>(graph: &MutableGraph<N, E>) -> Vec<NodeId> {
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

/// 强连通分量标签扫描（Kosaraju；仅有向图）。
///
/// 领域结论须经 engine 包装证书。
pub fn strongly_connected_components<N, E>(graph: &MutableGraph<N, E>) -> Result<Vec<NodeId>, GraphError> {
    if graph.direction() != GraphDirection::Directed {
        return Err(GraphError::UndirectedTopo);
    }
    let n = graph.node_count() as usize;
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut order = Vec::with_capacity(n);
    let mut seen = vec![false; n];
    for start in 0..n {
        if seen[start] {
            continue;
        }
        let mut stack = vec![(NodeId(start as u64), 0usize)];
        seen[start] = true;
        while let Some((node, idx)) = stack.pop() {
            let outs: Vec<NodeId> = graph.out_neighbors(node).collect();
            if idx < outs.len() {
                stack.push((node, idx + 1));
                let next = outs[idx];
                let i = next.0 as usize;
                if !seen[i] {
                    seen[i] = true;
                    stack.push((next, 0));
                }
            }
            else {
                order.push(node);
            }
        }
    }
    let mut labels = vec![NodeId(0); n];
    let mut seen = vec![false; n];
    for finish in order.into_iter().rev() {
        if seen[finish.0 as usize] {
            continue;
        }
        let root = finish;
        let mut stack = vec![finish];
        seen[finish.0 as usize] = true;
        labels[finish.0 as usize] = root;
        while let Some(node) = stack.pop() {
            for prev in graph.in_neighbors(node) {
                let i = prev.0 as usize;
                if !seen[i] {
                    seen[i] = true;
                    labels[i] = root;
                    stack.push(prev);
                }
            }
        }
    }
    Ok(labels)
}
