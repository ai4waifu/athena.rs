//! M-Graph / solver registry 合同测试。

use std::sync::Arc;

use athena_engine::reasoning::{
    mgraph::{CapabilityProviderId, ClosureLimits, ClosureStopReason, MGraphState, SolverCandidate, run_closure_step},
    solver::{ReflectionResult, Reflector, SolverContext, SolverRegistry, SolverRequest, score_candidate},
};
use athena_types::{Diagnostic, TermId};

struct StubReflector;

impl Reflector for StubReflector {
    fn reflect(&self, _state: &MGraphState, _request: &SolverRequest, _context: &SolverContext) -> Result<ReflectionResult, Diagnostic> {
        Ok(ReflectionResult::empty())
    }
}

#[test]
fn mgraph_state_defaults() {
    let store = athena_ir::TermStore::new();
    let mut state = MGraphState::new();
    assert!(state.operational.hyper_edges.is_empty());
    let result = run_closure_step(&store, &mut state, &ClosureLimits::default());
    assert_eq!(result.stop, ClosureStopReason::Saturated);
    assert!(result.is_saturated());
    assert_eq!(result.steps_applied, 0);
}

#[test]
fn solver_registry_lookup() {
    let mut registry = SolverRegistry::new();
    let id = CapabilityProviderId(1);
    registry.register(id, Arc::new(StubReflector));
    assert!(registry.get(id).is_ok());
    assert!(registry.get(CapabilityProviderId(99)).is_err());
}

#[test]
fn score_candidate_is_stable_integer() {
    let candidate = SolverCandidate { provider: CapabilityProviderId(0), roots: vec![TermId(0)] };
    let a = score_candidate(&candidate);
    let b = score_candidate(&candidate);
    assert_eq!(a, b);
    assert!(a.total >= 0);
    assert_ne!(a.tie_breaker, 0);
}

