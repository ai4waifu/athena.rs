//! Incremental M-Graph closure (Living `26` / `29` · bootstrap).
//!
//! Closure only propagates **already admitted** equality justifications into the
//! proof forest (transitivity). It never admits OuterCandidate / HyperEdge facts
//! and never invents witness-bearing claims.

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
    /// Hyper-edges drained into OuterCandidate pool this run.
    pub hyper_edges_staged: u32,
    /// Hyper-edges retained (unsupported mapping).
    pub hyper_edges_retained: u32,
}

impl ClosureResult {
    /// 是否在限额内完成饱和。
    pub fn is_saturated(&self) -> bool {
        matches!(self.stop, ClosureStopReason::Saturated)
    }
}

/// Bootstrap fragment:
/// 1. Drain stageable operational hyper-edges into OuterCandidate pool (no admit).
/// 2. Materialize [`ProofStepKind::Transitivity`] edges for one-hop compositions.
///
/// Does **not** write journal / ExactUF and does **not** promote OuterCandidate to facts.
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

#[cfg(test)]
mod tests {
    use athena_types::TermId;

    use crate::reasoning::mgraph::{
        AdmissionGate, CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, ProofStepKind, Proposition, Scope,
        VerificationPolicy,
    };

    use super::*;

    fn seed_equality(state: &mut MGraphState, left: u32, right: u32) {
        AdmissionGate::admit_claim(
            &mut state.semantic,
            Claim {
                proposition: Proposition::TermEquality { left: TermId(left), right: TermId(right) },
                scope: Scope::Unconditional,
                guarantee: Guarantee::ProvenExact,
                evidence: Evidence::TrustedKernel {
                    provider: CapabilityProviderId(0),
                    certificate: EvidenceCertificate::StructuralTermEquality { left: TermId(left), right: TermId(right) },
                    summary: "seed".into(),
                },
            },
            &VerificationPolicy::default(),
        )
        .expect("admit");
    }

    #[test]
    fn empty_state_is_already_saturated() {
        let store = athena_ir::TermStore::new();
        let mut state = MGraphState::new();
        let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
        assert_eq!(result.stop, ClosureStopReason::Saturated);
        assert_eq!(result.steps_applied, 0);
        assert_eq!(result.hyper_edges_staged, 0);
        assert!(result.is_saturated());
    }

    #[test]
    fn closure_materializes_transitivity_proof_edge() {
        let store = athena_ir::TermStore::new();
        let mut state = MGraphState::new();
        seed_equality(&mut state, 1, 2);
        seed_equality(&mut state, 2, 3);
        assert_eq!(state.semantic.derived.proof_forest.len(), 2);

        let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
        assert_eq!(result.stop, ClosureStopReason::Saturated);
        assert!(result.steps_applied >= 1);
        assert!(state.semantic.derived.proof_forest.edges().iter().any(|e| {
            e.step_kind == ProofStepKind::Transitivity
                && ((e.left == TermId(1) && e.right == TermId(3)) || (e.left == TermId(3) && e.right == TermId(1)))
        }));
        assert_eq!(state.semantic.derived.exact_uf.find(TermId(1)), state.semantic.derived.exact_uf.find(TermId(3)));
    }

    #[test]
    fn step_budget_stops_before_saturation() {
        let store = athena_ir::TermStore::new();
        let mut state = MGraphState::new();
        seed_equality(&mut state, 1, 2);
        seed_equality(&mut state, 2, 3);
        seed_equality(&mut state, 3, 4);
        let result = run_closure_step(&store, &mut state, &ClosureLimits { max_steps: 1 });
        assert_eq!(result.stop, ClosureStopReason::StepBudget);
        assert_eq!(result.steps_applied, 1);
        assert!(!result.is_saturated());
    }

    #[test]
    fn closure_drains_rewrite_hyper_edges_into_outer_pool() {
        use crate::reasoning::mgraph::{HyperEdge, predicates};
        use athena_ir::{Atom, TermNode};
        use athena_types::SourceSpan;

        let mut store = athena_ir::TermStore::new();
        let span = SourceSpan::default();
        let x = store.symbols_mut().intern("x");
        let y = store.symbols_mut().intern("y");
        let left = store.push(TermNode::Atom(Atom::Symbol(x)), span);
        let right = store.push(TermNode::Atom(Atom::Symbol(y)), span);

        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge { nodes: vec![left, right], predicate: predicates::REWRITE_EQUIVALENT });
        let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
        assert_eq!(result.hyper_edges_staged, 1);
        assert_eq!(result.hyper_edges_retained, 0);
        assert_eq!(state.operational.outer_candidates.len(), 1);
        assert!(state.operational.hyper_edges.is_empty());
        assert_eq!(state.semantic.relation_count(), 0);
    }
}
