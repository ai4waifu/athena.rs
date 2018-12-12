//! 内存图算法：BFS、拓扑排序、连通分量、SCC、union-find。

use std::collections::VecDeque;

use crate::{Graph, GraphDirection, GraphError, NodeId};

/// 确定性 BFS 访问顺序（有向图沿出边；无向图沿邻接）。
pub fn bfs_order<N, E>(graph: &Graph<N, E>, start: NodeId) -> Result<Vec<NodeId>, GraphError> {
    if start.0 >= graph.node_count() {
        return Err(GraphError::InvalidNode);
    }
    let mut seen = vec![false; graph.node_count() as usize];
    let mut order = Vec::new();
    let mut queue = VecDeque::from([start]);
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
    let mut ready: VecDeque<NodeId> = (0..n)
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

/// 强连通分量（仅有向图；无向图返回 [`GraphError::UndirectedTopo`]）。
///
/// 返回每个节点的 SCC 标签，标签为该分量中最小 `NodeId`（Kosaraju）。
pub fn strongly_connected_components<N, E>(graph: &Graph<N, E>) -> Result<Vec<NodeId>, GraphError> {
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
            } else {
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

/// 并查集（L0 结构原语）。
#[derive(Debug, Clone)]
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    /// 创建 `size` 个独立集合 `{0..size}`。
    pub fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
            rank: vec![0; size],
        }
    }

    /// 查找代表元（路径压缩）。
    pub fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    /// 合并集合；返回是否实际合并。
    pub fn union(&mut self, a: usize, b: usize) -> bool {
        let mut ra = self.find(a);
        let mut rb = self.find(b);
        if ra == rb {
            return false;
        }
        if self.rank[ra] < self.rank[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        if self.rank[ra] == self.rank[rb] {
            self.rank[ra] = self.rank[ra].saturating_add(1);
        }
        true
    }

    /// 当前集合个数。
    pub fn set_count(&mut self) -> usize {
        let n = self.parent.len();
        let mut roots = vec![false; n];
        for i in 0..n {
            roots[self.find(i)] = true;
        }
        roots.iter().filter(|&&b| b).count()
    }
}
