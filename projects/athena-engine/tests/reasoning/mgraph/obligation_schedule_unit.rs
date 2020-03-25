//! 自 `src/reasoning/mgraph/obligation/schedule.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::planner::{DomainPlan, PlanStep},
    reasoning::mgraph::{
        FactId, MGraphCore, MGraphState, MGraphView, PredicateId, ProofObligation, ReflectorWake, ScopeRef, SemanticReflector, obligation::*,
    },
};

struct AlwaysKnown;

impl SemanticReflector for AlwaysKnown {
    fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
        Reflection::AlreadyKnown { relation: FactId(0) }
    }
}

struct AlwaysInconclusive;

impl SemanticReflector for AlwaysInconclusive {
    fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
        Reflection::Inconclusive
    }
}

struct AlwaysCompute;

impl SemanticReflector for AlwaysCompute {
    fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
        Reflection::NeedComputation { plan: DomainPlan { steps: vec![PlanStep::CallDomainProvider] } }
    }
}

fn sample_wake() -> ReflectorWake {
    ReflectorWake {
        obligation: ProofObligation { predicate: PredicateId(1), scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] },
        relation: FactId(0),
    }
}

#[test]
fn schedule_already_known_counts() {
    let mut state = MGraphState::new();
    let report = schedule_reflector_wakes(&mut state, &[sample_wake()], &AlwaysKnown);
    assert_eq!(report.already_known, 1);
    assert!(state.operational.pending_plans.is_empty());
    assert!(state.operational.resume_queue.is_empty());
    let _ = MGraphCore::new();
}

#[test]
fn schedule_need_computation_queues_plan() {
    let mut state = MGraphState::new();
    let report = schedule_reflector_wakes(&mut state, &[sample_wake()], &AlwaysCompute);
    assert_eq!(report.need_computation, 1);
    assert_eq!(state.operational.pending_plans.len(), 1);
    assert_eq!(state.operational.pending_plans[0].obligation.predicate, PredicateId(1));
}

#[test]
fn resume_frontier_requeues_inconclusive() {
    let mut state = MGraphState::new();
    state.operational.resume_queue.push(sample_wake().obligation);
    let report = resume_reflector_frontier(&mut state, &AlwaysInconclusive);
    assert_eq!(report.inconclusive_resumed, 1);
    assert_eq!(state.operational.resume_queue.len(), 1);
}

#[test]
fn resume_frontier_clears_when_known() {
    let mut state = MGraphState::new();
    state.operational.resume_queue.push(sample_wake().obligation);
    let report = resume_reflector_frontier(&mut state, &AlwaysKnown);
    assert_eq!(report.already_known, 1);
    assert!(state.operational.resume_queue.is_empty());
}
