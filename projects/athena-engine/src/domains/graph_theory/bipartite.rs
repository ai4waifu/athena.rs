//! 二部性判定（BFS 二染色）。

use std::collections::{HashMap, VecDeque};

use athena_graph::NodeId;

use super::{
    object::{GraphNodeId, GraphObject},
    property::{GraphCertificate, GraphPropertyKind, GraphPropertyResult},
    result::BipartiteResult,
};

/// 判定图是否二部；非二部时返回奇环证书。
///
/// 有向图按底层无向邻接解释（与弱连通一致）。
pub fn bipartite_l1(graph: &GraphObject) -> BipartiteResult {
    let n = graph.node_count() as usize;
    let undirected = build_undirected_adj(graph, n);
    let mut color = vec![-1i8; n];
    let mut parent = vec![None; n];
    let mut left = Vec::new();
    let mut right = Vec::new();

    for start in 0..n {
        if color[start] != -1 {
            continue;
        }
        color[start] = 0;
        let mut queue = VecDeque::from([start]);
        while let Some(u) = queue.pop_front() {
            if color[u] == 0 {
                left.push(NodeId(u as u64));
            }
            else {
                right.push(NodeId(u as u64));
            }
            for &v in &undirected[u] {
                if color[v] == -1 {
                    color[v] = 1 - color[u];
                    parent[v] = Some(u);
                    queue.push_back(v);
                }
                else if color[v] == color[u] {
                    let cycle = reconstruct_odd_cycle(u, v, &parent);
                    return BipartiteResult::NotBipartite {
                        property: GraphPropertyResult::disproven(
                            GraphPropertyKind::Bipartiteness,
                            (),
                            "bfs_2color",
                            GraphCertificate::OddCycle { algorithm: "bfs_2color", cycle },
                            graph.snapshot.clone(),
                        ),
                    };
                }
            }
        }
    }

    BipartiteResult::Bipartite {
        left: left.clone(),
        right: right.clone(),
        property: GraphPropertyResult::proven(
            GraphPropertyKind::Bipartiteness,
            true,
            "bfs_2color",
            GraphCertificate::BipartiteColoring { algorithm: "bfs_2color", left, right },
            graph.snapshot.clone(),
        ),
    }
}

fn build_undirected_adj(graph: &GraphObject, n: usize) -> Vec<Vec<usize>> {
    let mut adj = vec![Vec::new(); n];
    for (s, t, _) in &graph.memory.edges {
        let u = s.0 as usize;
        let v = t.0 as usize;
        if u >= n || v >= n || u == v {
            continue;
        }
        adj[u].push(v);
        adj[v].push(u);
    }
    for list in &mut adj {
        list.sort_unstable();
        list.dedup();
    }
    adj
}

fn reconstruct_odd_cycle(u: usize, v: usize, parent: &[Option<usize>]) -> Vec<GraphNodeId> {
    let mut depth_u = HashMap::new();
    let mut cur = Some(u);
    let mut d = 0usize;
    while let Some(node) = cur {
        depth_u.insert(node, d);
        cur = parent[node];
        d += 1;
    }
    let mut path_v = Vec::new();
    let mut cur = Some(v);
    let mut lca = v;
    while let Some(node) = cur {
        path_v.push(node);
        if depth_u.contains_key(&node) {
            lca = node;
            break;
        }
        cur = parent[node];
    }
    let mut path_u = Vec::new();
    let mut cur = Some(u);
    while let Some(node) = cur {
        path_u.push(node);
        if node == lca {
            break;
        }
        cur = parent[node];
    }
    let mut cycle: Vec<GraphNodeId> = path_u.iter().map(|&x| NodeId(x as u64)).collect();
    for &x in path_v.iter().rev().skip(1) {
        cycle.push(NodeId(x as u64));
    }
    if let Some(&first) = cycle.first() {
        if cycle.last() != Some(&first) {
            cycle.push(first);
        }
    }
    cycle
}
