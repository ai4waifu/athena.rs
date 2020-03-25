//! 增量 M-Graph 闭包（引导实现）。
//!
//! 闭包仅把 **已接纳** 的等式理据传播进
//! 证明森林（传递性）。绝不接纳 OuterCandidate / HyperEdge 事实，
//! 也绝不伪造带见证的声明。

pub mod drain;
pub mod operational;

use athena_ir::TermStore;
use athena_types::TermId;

use crate::reasoning::mgraph::{ProofForest, core::state::MGraphState, equivalence::proof_forest::ProofStepKind};

pub use drain::{HyperEdgeDrainReport, drain_hyper_edges_to_outer_pool};
pub use operational::OperationalState;

/// 闭包资源限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosureLimits {
    /// 最大步数（每步至多物化一条传递性证明边）。
    pub max_steps: u32,
}

impl Default for ClosureLimits {
    fn default() -> Self {
        Self { max_steps: 1024 }
    }
}

/// 闭包停止原因（终态枚举 · 禁止 `complete: bool` 冒充饱和语义）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClosureStopReason {
    /// 在限额内达到饱和（无可再物化的传递性边）。
    Saturated,
    /// 步数预算耗尽（仍有可物化工作）。
    StepBudget,
}

/// 闭包结果摘要（状态已就地更新）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClosureResult {
    /// 停止原因。
    pub stop: ClosureStopReason,
    /// 本轮物化的传递性边数。
    pub steps_applied: u32,
    /// 本次运行排入 OuterCandidate 池的超边数。
    pub hyper_edges_staged: u32,
    /// 仍保留的超边（映射不支持）。
    pub hyper_edges_retained: u32,
}

impl ClosureResult {
    /// 是否在限额内完成饱和。
    pub fn is_saturated(&self) -> bool {
        matches!(self.stop, ClosureStopReason::Saturated)
    }
}

/// 引导片段：
/// 1. 将可暂存操作超边排入 OuterCandidate 池（不接纳）。
/// 2. 为一跳复合物化 [`ProofStepKind::Transitivity`] 边。
///
/// **不会** 写入 journal / ExactUF，也 **不会** 将 OuterCandidate 提升为事实。
pub fn run_closure_step(store: &TermStore, state: &mut MGraphState, limits: &ClosureLimits) -> ClosureResult {
    let drain = drain_hyper_edges_to_outer_pool(store, state);
    let mut steps_applied = 0u32;

    while steps_applied < limits.max_steps {
        match materialize_one_transitivity_edge(state) {
            true => steps_applied = steps_applied.saturating_add(1),
            false => {
                return ClosureResult {
                    stop: ClosureStopReason::Saturated,
                    steps_applied,
                    hyper_edges_staged: drain.staged,
                    hyper_edges_retained: drain.retained,
                };
            }
        }
    }

    let more = pending_transitivity_exists(state);
    ClosureResult {
        stop: if more { ClosureStopReason::StepBudget } else { ClosureStopReason::Saturated },
        steps_applied,
        hyper_edges_staged: drain.staged,
        hyper_edges_retained: drain.retained,
    }
}

fn materialize_one_transitivity_edge(state: &mut MGraphState) -> bool {
    let Some((left, right)) = next_transitivity_pair(state)
    else {
        return false;
    };
    state.semantic.derived.proof_forest.record(left, right, ProofStepKind::Transitivity);
    true
}

fn pending_transitivity_exists(state: &MGraphState) -> bool {
    next_transitivity_pair(state).is_some()
}

fn next_transitivity_pair(state: &MGraphState) -> Option<(TermId, TermId)> {
    let forest = &state.semantic.derived.proof_forest;
    let edges = forest.edges();
    if edges.len() < 2 {
        return None;
    }

    let mut adj: Vec<(TermId, TermId)> = Vec::with_capacity(edges.len() * 2);
    for e in edges {
        adj.push((e.left, e.right));
        adj.push((e.right, e.left));
    }

    for &(a, b) in &adj {
        for &(b2, c) in &adj {
            if b != b2 || a == c {
                continue;
            }
            if state.semantic.derived.exact_uf.find(a) != state.semantic.derived.exact_uf.find(c) {
                continue;
            }
            let (left, right) = if a.0 <= c.0 { (a, c) } else { (c, a) };
            if forest_has_pair(forest, left, right) {
                continue;
            }
            return Some((left, right));
        }
    }
    None
}

fn forest_has_pair(forest: &ProofForest, left: TermId, right: TermId) -> bool {
    forest.edges().iter().any(|e| (e.left == left && e.right == right) || (e.left == right && e.right == left))
}
