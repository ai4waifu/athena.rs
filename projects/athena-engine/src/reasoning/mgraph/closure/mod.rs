//! Incremental M-Graph closure (Living `26` / `29` · bootstrap).
//!
//! Closure only propagates **already admitted** equality justifications into the
//! proof forest (transitivity). It never admits OuterCandidate / HyperEdge facts
//! and never invents witness-bearing claims.

pub mod operational;

use athena_types::TermId;

use crate::reasoning::mgraph::{
    core::state::MGraphState,
    equivalence::proof_forest::ProofStepKind,
    ProofForest,
};

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
}

impl ClosureResult {
    /// 是否在限额内完成饱和。
    pub fn is_saturated(&self) -> bool {
        matches!(self.stop, ClosureStopReason::Saturated)
    }
}

/// 就地运行闭包直至饱和或步数预算耗尽。
///
/// Bootstrap fragment: materialize [`ProofStepKind::Transitivity`] edges for
/// one-hop compositions of existing proof-forest equalities when ExactUF already
/// equates the endpoints. Does **not** write journal / ExactUF (already closed
/// under union) and does **not** promote operational hyper-edges.
pub fn run_closure_step(state: &mut MGraphState, limits: &ClosureLimits) -> ClosureResult {
    let mut steps_applied = 0u32;

    while steps_applied < limits.max_steps {
        match materialize_one_transitivity_edge(state) {
            true => steps_applied = steps_applied.saturating_add(1),
            false => {
                return ClosureResult {
                    stop: ClosureStopReason::Saturated,
                    steps_applied,
                };
            }
        }
    }

    let more = pending_transitivity_exists(state);
    ClosureResult {
        stop: if more {
            ClosureStopReason::StepBudget
        } else {
            ClosureStopReason::Saturated
        },
        steps_applied,
    }
}

fn materialize_one_transitivity_edge(state: &mut MGraphState) -> bool {
    let Some((left, right)) = next_transitivity_pair(state) else {
        return false;
    };
    state
        .semantic
        .derived
        .proof_forest
        .record(left, right, ProofStepKind::Transitivity);
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
    forest.edges().iter().any(|e| {
        (e.left == left && e.right == right) || (e.left == right && e.right == left)
    })
}

#[cfg(test)]
mod tests {
    use athena_types::TermId;

    use crate::reasoning::mgraph::{
        AdmissionGate, CapabilityProviderId, Claim, Evidence, EvidenceCertificate, Guarantee, Proposition,
        Scope, VerificationPolicy, ProofStepKind,
    };

    use super::*;

    fn seed_equality(state: &mut MGraphState, left: u32, right: u32) {
        AdmissionGate::admit_claim(
            &mut state.semantic,
            Claim {
                proposition: Proposition::TermEquality {
                    left: TermId(left),
                    right: TermId(right),
                },
                scope: Scope::Unconditional,
                guarantee: Guarantee::ProvenExact,
                evidence: Evidence::TrustedKernel {
                    provider: CapabilityProviderId(0),
                    certificate: EvidenceCertificate::StructuralTermEquality {
                        left: TermId(left),
                        right: TermId(right),
                    },
                    summary: "seed".into(),
                },
            },
            &VerificationPolicy::default(),
        )
        .expect("admit");
    }

    #[test]
    fn empty_state_is_already_saturated() {
        let mut state = MGraphState::new();
        let result = run_closure_step(&mut state, &ClosureLimits::default());
        assert_eq!(result.stop, ClosureStopReason::Saturated);
        assert_eq!(result.steps_applied, 0);
        assert!(result.is_saturated());
    }

    #[test]
    fn closure_materializes_transitivity_proof_edge() {
        let mut state = MGraphState::new();
        seed_equality(&mut state, 1, 2);
        seed_equality(&mut state, 2, 3);
        assert_eq!(state.semantic.derived.proof_forest.len(), 2);

        let result = run_closure_step(&mut state, &ClosureLimits::default());
        assert_eq!(result.stop, ClosureStopReason::Saturated);
        assert!(result.steps_applied >= 1);
        assert!(state.semantic.derived.proof_forest.edges().iter().any(|e| {
            e.step_kind == ProofStepKind::Transitivity
                && ((e.left == TermId(1) && e.right == TermId(3))
                    || (e.left == TermId(3) && e.right == TermId(1)))
        }));
        assert_eq!(
            state.semantic.derived.exact_uf.find(TermId(1)),
            state.semantic.derived.exact_uf.find(TermId(3))
        );
    }

    #[test]
    fn step_budget_stops_before_saturation() {
        let mut state = MGraphState::new();
        seed_equality(&mut state, 1, 2);
        seed_equality(&mut state, 2, 3);
        seed_equality(&mut state, 3, 4);
        let result = run_closure_step(&mut state, &ClosureLimits { max_steps: 1 });
        assert_eq!(result.stop, ClosureStopReason::StepBudget);
        assert_eq!(result.steps_applied, 1);
        assert!(!result.is_saturated());
    }
}
