//! Budgeted saturation driver (bootstrap · rule matching not wired yet).

use athena_ir::TermStore;
use athena_rewriter::RuleSet;
use athena_types::TermId;

use super::{
    budget::{SaturationBudget, SaturationStopReason},
    candidate::CandidateEquivalence,
    graph::EGraph,
};

/// Report from one saturation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaturationReport {
    /// Why the run stopped.
    pub stop: SaturationStopReason,
    /// Iterations performed.
    pub iterations: u32,
    /// Candidate equalities discovered (unverified).
    pub candidates: Vec<CandidateEquivalence>,
}

/// Run scope-local saturation under `budget`.
///
/// Bootstrap behavior: add all `roots`, then stop at fixed point.
/// `rules` is accepted for API stability; matching lands in a later fill-in.
pub fn saturate(
    graph: &mut EGraph,
    store: &TermStore,
    roots: &[TermId],
    budget: SaturationBudget,
    rules: Option<&RuleSet>,
) -> SaturationReport {
    let _ = rules;
    if budget.max_iterations == 0 || budget.max_eclasses == 0 || budget.max_enodes == 0 {
        return SaturationReport {
            stop: SaturationStopReason::ResourceBudget,
            iterations: 0,
            candidates: Vec::new(),
        };
    }

    let mut iterations = 0;
    for root in roots {
        if graph.eclass_count() as u32 >= budget.max_eclasses
            || graph.enode_count() as u32 >= budget.max_enodes
        {
            return SaturationReport {
                stop: SaturationStopReason::ResourceBudget,
                iterations,
                candidates: Vec::new(),
            };
        }
        let _ = graph.add_term(store, *root);
        iterations = iterations.saturating_add(1);
        if iterations >= budget.max_iterations {
            return SaturationReport {
                stop: SaturationStopReason::IterationBudget,
                iterations,
                candidates: Vec::new(),
            };
        }
    }

    SaturationReport {
        stop: SaturationStopReason::FixedPoint,
        iterations,
        candidates: Vec::new(),
    }
}
