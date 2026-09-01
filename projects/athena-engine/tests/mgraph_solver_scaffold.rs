//! M-Graph / solver 骨架冒烟测试。

use std::sync::Arc;

use athena_engine::{
    ClosureLimits, Diagnostic, DomainRef, MGraphState, ReflectionResult, Reflector, SolverCandidate, SolverContext, SolverId,
    SolverLimits, SolverOperation, SolverRegistry, SolverRequest, run_closure_step, score_candidate,
};
use athena_types::{AssumptionSetId, TermId};

struct StubReflector;

impl Reflector for StubReflector {
    fn reflect(
        &self,
        _state: &MGraphState,
        _request: &SolverRequest,
        _context: &SolverContext,
    ) -> Result<ReflectionResult, Diagnostic> {
        Ok(ReflectionResult::empty())
    }
}

#[test]
fn mgraph_state_defaults() {
    let state = MGraphState::new();
    assert!(state.hyper_edges.is_empty());
    let result = run_closure_step(&state, &ClosureLimits::default());
    assert!(!result.complete);
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn solver_registry_lookup() {
    let mut registry = SolverRegistry::new();
    let id = SolverId(1);
    registry.register(id, Arc::new(StubReflector));
    assert!(registry.get(id).is_ok());
    assert!(registry.get(SolverId(99)).is_err());
}

#[test]
fn score_candidate_is_finite() {
    let score = score_candidate(&SolverCandidate { solver: SolverId(0), roots: vec![TermId(0)] });
    assert!(score.estimated_benefit.is_finite());
}

#[test]
fn solver_request_scaffold() {
    let req = SolverRequest {
        domain: DomainRef::Arithmetic,
        roots: vec![TermId(1)],
        operation: SolverOperation { name: "noop".into() },
        limits: SolverLimits::default(),
        assumptions: AssumptionSetId(0),
    };
    assert_eq!(req.operation.name, "noop");
}
