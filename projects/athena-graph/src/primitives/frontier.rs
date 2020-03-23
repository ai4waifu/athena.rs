//! 确定性 frontier、可恢复检查点与取消标志。
//!
//! **稳定序合同**（同输入 → 同中间顺序）：
//! - frontier 为 FIFO（`VecDeque`）
//! - 扩展某节点时，邻接目标按 [`NodeId`] 升序入队（与边插入顺序无关）
//! - 初始种子按调用方给定顺序；多源时调用方须自行排序种子

use std::collections::VecDeque;

use crate::{GraphError, NodeId};

/// 协作式取消标志（单线程算法轮询）。
#[derive(Debug, Default, Clone, Copy)]
pub struct CancelFlag {
    cancelled: bool,
}

impl CancelFlag {
    /// 新建未取消标志。
    pub const fn new() -> Self {
        Self { cancelled: false }
    }

    /// 请求取消。
    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    /// 是否已请求取消。
    pub const fn is_cancelled(&self) -> bool {
        self.cancelled
    }
}

/// 可恢复 BFS frontier 检查点。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct FrontierCheckpoint {
    /// 待处理队列（FIFO 前端在前）。
    pub queue: Vec<NodeId>,
    /// 已发现（含已出队与仍在队列中）的位图，下标 = `NodeId.0`。
    pub discovered: Vec<bool>,
    /// 已输出的访问前缀。
    pub visited_prefix: Vec<NodeId>,
}

impl FrontierCheckpoint {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self { queue: self.queue.clone(), discovered: self.discovered.clone(), visited_prefix: self.visited_prefix.clone() }
    }
}

/// 确定性 FIFO frontier。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default)]
pub struct DeterministicFrontier {
    queue: VecDeque<NodeId>,
}

impl DeterministicFrontier {
    /// 空 frontier。
    pub fn new() -> Self {
        Self { queue: VecDeque::new() }
    }

    /// Owning 复制（Living `31`：队列）。
    pub fn owning_copy(&self) -> Self {
        Self {
            queue: self.queue.clone(),
        }
    }

    /// 从检查点恢复队列（不恢复 `discovered` / 前缀，由调用方持有）。
    pub fn from_queue(nodes: impl IntoIterator<Item = NodeId>) -> Self {
        Self { queue: nodes.into_iter().collect() }
    }

    /// 队尾入队。
    pub fn push_back(&mut self, node: NodeId) {
        self.queue.push_back(node);
    }

    /// 队头出队。
    pub fn pop_front(&mut self) -> Option<NodeId> {
        self.queue.pop_front()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// 当前长度。
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 导出队列快照（前端在前）。
    pub fn as_slice_order(&self) -> Vec<NodeId> {
        self.queue.iter().copied().collect()
    }
}

/// 将邻接目标按 [`NodeId`] 升序排序（确定性扩展序）。
pub fn sort_neighbors_deterministic(neighbors: &mut [NodeId]) {
    neighbors.sort_unstable();
}

/// 确定性 BFS 结果。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub enum DeterministicBfsOutcome {
    /// 完整访问序（仅可达子图）。
    Complete(Vec<NodeId>),
    /// 取消时的部分结果与检查点（可 [`resume_deterministic_bfs`]）。
    Cancelled {
        /// 部分访问序。
        partial: Vec<NodeId>,
        /// 恢复点。
        checkpoint: FrontierCheckpoint,
    },
}

impl DeterministicBfsOutcome {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Complete(order) => Self::Complete(order.clone()),
            Self::Cancelled { partial, checkpoint } => {
                Self::Cancelled { partial: partial.clone(), checkpoint: checkpoint.owning_copy() }
            }
        }
    }
}

fn run_deterministic_bfs_loop<N, E>(
    graph: &crate::MutableGraph<N, E>,
    discovered: &mut [bool],
    visited: &mut Vec<NodeId>,
    frontier: &mut DeterministicFrontier,
    cancel: Option<&CancelFlag>,
) -> Result<DeterministicBfsOutcome, GraphError> {
    while let Some(node) = frontier.pop_front() {
        if cancel.is_some_and(CancelFlag::is_cancelled) {
            let mut queue = frontier.as_slice_order();
            queue.insert(0, node);
            return Ok(DeterministicBfsOutcome::Cancelled {
                partial: visited.clone(),
                checkpoint: FrontierCheckpoint { queue, discovered: discovered.to_vec(), visited_prefix: visited.clone() },
            });
        }
        visited.push(node);
        let mut nexts: Vec<NodeId> = graph.out_neighbors(node).collect();
        sort_neighbors_deterministic(&mut nexts);
        for next in nexts {
            let i = next.0 as usize;
            if !discovered[i] {
                discovered[i] = true;
                frontier.push_back(next);
            }
        }
    }
    Ok(DeterministicBfsOutcome::Complete(visited.clone()))
}

/// 在内存图上执行确定性 BFS（邻接按 `NodeId` 升序扩展）。
pub fn deterministic_bfs<N, E>(
    graph: &crate::MutableGraph<N, E>,
    start: NodeId,
    cancel: Option<&CancelFlag>,
) -> Result<DeterministicBfsOutcome, GraphError> {
    if start.0 >= graph.node_count() {
        return Err(GraphError::InvalidNode);
    }
    let n = graph.node_count() as usize;
    let mut discovered = vec![false; n];
    let mut visited = Vec::new();
    let mut frontier = DeterministicFrontier::new();
    discovered[start.0 as usize] = true;
    frontier.push_back(start);
    run_deterministic_bfs_loop(graph, &mut discovered, &mut visited, &mut frontier, cancel)
}

/// 从检查点恢复确定性 BFS。
pub fn resume_deterministic_bfs<N, E>(
    graph: &crate::MutableGraph<N, E>,
    checkpoint: FrontierCheckpoint,
    cancel: Option<&CancelFlag>,
) -> Result<DeterministicBfsOutcome, GraphError> {
    let n = graph.node_count() as usize;
    if checkpoint.discovered.len() != n {
        return Err(GraphError::InvalidNode);
    }
    let mut discovered = checkpoint.discovered;
    let mut visited = checkpoint.visited_prefix;
    let mut frontier = DeterministicFrontier::from_queue(checkpoint.queue);
    run_deterministic_bfs_loop(graph, &mut discovered, &mut visited, &mut frontier, cancel)
}
