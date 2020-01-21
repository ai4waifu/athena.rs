//! Schedule Reflector wakes and frontier resume (Living `29` · bootstrap).

use crate::reasoning::mgraph::{
    core::state::MGraphState,
    obligation::{ProofObligation, Reflection, ReflectorWake, SemanticReflector},
};

/// Counts from applying Reflector outcomes to operational queues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReflectorScheduleReport {
    /// Wakes that resolved to [`Reflection::AlreadyKnown`].
    pub already_known: u32,
    /// Plans queued for domain execution.
    pub need_computation: u32,
    /// Nested obligations re-registered in the obligation index.
    pub need_relation: u32,
    /// Object gaps (counted; not queued in bootstrap).
    pub need_object: u32,
    /// Conversion gaps (counted; not queued in bootstrap).
    pub need_conversion: u32,
    /// Obligations pushed to the resume queue for later re-reflect.
    pub inconclusive_resumed: u32,
}

/// Apply Reflector outcomes for a batch of wakes into operational queues.
///
/// Does **not** admit facts and does **not** call `execute_domain`.
pub fn schedule_reflector_wakes(
    state: &mut MGraphState,
    wakes: &[ReflectorWake],
    reflector: &dyn SemanticReflector,
) -> ReflectorScheduleReport {
    let outcomes: Vec<Reflection> = {
        let view = state.semantic.view();
        wakes
            .iter()
            .map(|wake| reflector.reflect(&wake.obligation, &view))
            .collect()
    };
    apply_reflections(state, wakes.iter().map(|w| &w.obligation), outcomes)
}

/// Re-reflect obligations drained from the resume queue (frontier resume).
pub fn resume_reflector_frontier(
    state: &mut MGraphState,
    reflector: &dyn SemanticReflector,
) -> ReflectorScheduleReport {
    let pending = std::mem::take(&mut state.operational.resume_queue);
    let outcomes: Vec<Reflection> = {
        let view = state.semantic.view();
        pending
            .iter()
            .map(|obligation| reflector.reflect(obligation, &view))
            .collect()
    };
    apply_reflections(state, pending.iter(), outcomes)
}

fn apply_reflections<'a>(
    state: &mut MGraphState,
    obligations: impl Iterator<Item = &'a ProofObligation>,
    outcomes: Vec<Reflection>,
) -> ReflectorScheduleReport {
    let mut report = ReflectorScheduleReport::default();
    for (obligation, outcome) in obligations.zip(outcomes) {
        match outcome {
            Reflection::AlreadyKnown { .. } => {
                report.already_known = report.already_known.saturating_add(1);
            }
            Reflection::NeedComputation { plan } => {
                state.operational.pending_plans.push(plan);
                report.need_computation = report.need_computation.saturating_add(1);
            }
            Reflection::NeedRelation { obligation: nested } => {
                state.operational.obligation_index.register(nested);
                report.need_relation = report.need_relation.saturating_add(1);
            }
            Reflection::NeedObject { .. } => {
                report.need_object = report.need_object.saturating_add(1);
            }
            Reflection::NeedConversion { .. } => {
                report.need_conversion = report.need_conversion.saturating_add(1);
            }
            Reflection::Inconclusive => {
                state.operational.resume_queue.push(obligation.clone());
                report.inconclusive_resumed = report.inconclusive_resumed.saturating_add(1);
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domains::planner::{PlanStep, DomainPlan},
        reasoning::mgraph::{
            MGraphCore, MGraphView, PredicateId, ProofObligation, ScopeRef, SemanticReflector,
            ReflectorWake, FactId,
        },
    };

    struct AlwaysKnown;

    impl SemanticReflector for AlwaysKnown {
        fn reflect(&self, _obligation: &ProofObligation, _view: &MGraphView<'_>) -> Reflection {
            Reflection::AlreadyKnown {
                relation: FactId(0),
            }
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
            Reflection::NeedComputation {
                plan: DomainPlan {
                    steps: vec![PlanStep::CallDomainProvider],
                },
            }
        }
    }

    fn sample_wake() -> ReflectorWake {
        ReflectorWake {
            obligation: ProofObligation {
                predicate: PredicateId(1),
                scope: ScopeRef::UNCONDITIONAL,
                known_objects: vec![],
            },
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
}
