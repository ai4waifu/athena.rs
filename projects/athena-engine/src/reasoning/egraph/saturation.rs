//! Budgeted saturation driver (Living `03` R-2.5 / `26`).

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
/// Bootstrap matcher: after ingesting `roots`, scan known terms for
/// [`TermStore::structural_eq`] hits against each rule pattern, add the
/// replacement, emit a [`CandidateEquivalence`], and optionally union classes
/// locally. Never writes M-Graph.
pub fn saturate(
    graph: &mut EGraph,
    store: &TermStore,
    roots: &[TermId],
    budget: SaturationBudget,
    rules: Option<&RuleSet>,
) -> SaturationReport {
    if budget.max_iterations == 0 || budget.max_eclasses == 0 || budget.max_enodes == 0 {
        return SaturationReport {
            stop: SaturationStopReason::ResourceBudget,
            iterations: 0,
            candidates: Vec::new(),
        };
    }

    let mut iterations = 0u32;
    let mut candidates = Vec::new();

    for root in roots {
        if over_structure_budget(graph, &budget) {
            return SaturationReport {
                stop: SaturationStopReason::ResourceBudget,
                iterations,
                candidates,
            };
        }
        let _ = graph.add_term(store, *root);
        iterations = iterations.saturating_add(1);
        if iterations >= budget.max_iterations {
            return SaturationReport {
                stop: SaturationStopReason::IterationBudget,
                iterations,
                candidates,
            };
        }
    }

    let Some(rules) = rules else {
        return SaturationReport {
            stop: SaturationStopReason::FixedPoint,
            iterations,
            candidates,
        };
    };

    if rules.is_empty() {
        return SaturationReport {
            stop: SaturationStopReason::FixedPoint,
            iterations,
            candidates,
        };
    }

    loop {
        if iterations >= budget.max_iterations {
            return SaturationReport {
                stop: SaturationStopReason::IterationBudget,
                iterations,
                candidates,
            };
        }
        if over_structure_budget(graph, &budget) {
            return SaturationReport {
                stop: SaturationStopReason::ResourceBudget,
                iterations,
                candidates,
            };
        }

        let mut progressed = false;
        let subjects = graph.known_terms();
        for rule in rules.iter() {
            for &subject in &subjects {
                if candidates.len() as u32 >= budget.max_candidate_unions {
                    return SaturationReport {
                        stop: SaturationStopReason::ResourceBudget,
                        iterations,
                        candidates,
                    };
                }
                if !store.structural_eq(subject, rule.pattern) {
                    continue;
                }
                let Some(left_class) = graph.class_of_term(subject) else {
                    continue;
                };
                let Some(right_class) = graph.add_term(store, rule.replacement) else {
                    continue;
                };
                if graph.find(left_class) == graph.find(right_class) {
                    continue;
                }
                if already_emitted(&candidates, subject, rule.replacement) {
                    continue;
                }
                graph.union_classes(left_class, right_class);
                candidates.push(CandidateEquivalence {
                    left_term: subject,
                    right_term: rule.replacement,
                    left_class,
                    right_class,
                });
                progressed = true;
            }
        }

        iterations = iterations.saturating_add(1);
        if !progressed {
            return SaturationReport {
                stop: SaturationStopReason::FixedPoint,
                iterations,
                candidates,
            };
        }
    }
}

fn over_structure_budget(graph: &EGraph, budget: &SaturationBudget) -> bool {
    graph.eclass_count() as u32 >= budget.max_eclasses || graph.enode_count() as u32 >= budget.max_enodes
}

fn already_emitted(candidates: &[CandidateEquivalence], left: TermId, right: TermId) -> bool {
    candidates.iter().any(|c| {
        (c.left_term == left && c.right_term == right) || (c.left_term == right && c.right_term == left)
    })
}
