//! Drain operational hyper-edges into the OuterCandidate pool (Living `26`).

use athena_ir::TermStore;

use crate::reasoning::mgraph::{
    admission::hyper_edge_to_outer_candidate,
    core::state::MGraphState,
};

use super::operational::OperationalState;

/// Report from draining operational hyper-edges into the OuterCandidate pool.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HyperEdgeDrainReport {
    /// Edges successfully mapped to outer candidates.
    pub staged: u32,
    /// Edges retained in `hyper_edges` (unsupported / not yet mappable).
    pub retained: u32,
}

/// Move stageable operational hyper-edges into [`OperationalState::outer_candidates`].
///
/// Unsupported edges stay in `hyper_edges`. Never admits into SemanticCore / ExactUF.
pub fn drain_hyper_edges_to_outer_pool(
    store: &TermStore,
    state: &mut MGraphState,
) -> HyperEdgeDrainReport {
    drain_operational_hyper_edges(store, &mut state.operational)
}

fn drain_operational_hyper_edges(
    store: &TermStore,
    operational: &mut OperationalState,
) -> HyperEdgeDrainReport {
    let pending = std::mem::take(&mut operational.hyper_edges);
    let mut retained = Vec::new();
    let mut staged = 0u32;
    for edge in pending {
        match hyper_edge_to_outer_candidate(store, &edge) {
            Ok(outer) => {
                operational.outer_candidates.push(outer);
                staged = staged.saturating_add(1);
            }
            Err(_) => retained.push(edge),
        }
    }
    let retained_count = retained.len() as u32;
    operational.hyper_edges = retained;
    HyperEdgeDrainReport {
        staged,
        retained: retained_count,
    }
}

#[cfg(test)]
mod tests {
    use athena_ir::{Atom, TermNode};
    use athena_types::SourceSpan;

    use crate::reasoning::mgraph::{HyperEdge, MGraphState, predicates};

    use super::*;

    fn push_symbol(store: &mut TermStore, name: &str) -> athena_types::TermId {
        let span = SourceSpan::default();
        let sym = store.symbols_mut().intern(name);
        store.push(TermNode::Atom(Atom::Symbol(sym)), span)
    }

    #[test]
    fn drain_moves_rewrite_edges_without_admitting() {
        use crate::reasoning::mgraph::PredicateId;

        let mut store = TermStore::new();
        let a = push_symbol(&mut store, "a");
        let b = push_symbol(&mut store, "b");
        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![a, b],
            predicate: predicates::REWRITE_EQUIVALENT,
        });
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![a],
            predicate: PredicateId(99),
        });
        let report = drain_hyper_edges_to_outer_pool(&store, &mut state);
        assert_eq!(report.staged, 1);
        assert_eq!(report.retained, 1);
        assert_eq!(state.operational.outer_candidates.len(), 1);
        assert_eq!(state.operational.hyper_edges.len(), 1);
        assert_eq!(state.semantic.derived.exact_uf.union_count(), 0);
        assert_eq!(state.semantic.relation_count(), 0);
    }

    #[test]
    fn drain_moves_polynomial_result_edges() {
        let mut store = TermStore::new();
        let req = push_symbol(&mut store, "poly_req");
        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![req],
            predicate: predicates::POLYNOMIAL_RESULT,
        });
        let report = drain_hyper_edges_to_outer_pool(&store, &mut state);
        assert_eq!(report.staged, 1);
        assert_eq!(report.retained, 0);
        assert!(state.operational.hyper_edges.is_empty());
        assert_eq!(state.semantic.relation_count(), 0);
    }

    #[test]
    fn drain_moves_evaluation_result_edges() {
        let mut store = TermStore::new();
        let a = push_symbol(&mut store, "x");
        let b = push_symbol(&mut store, "y");
        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![a, b],
            predicate: predicates::EVALUATION_RESULT,
        });
        let report = drain_hyper_edges_to_outer_pool(&store, &mut state);
        assert_eq!(report.staged, 1);
        assert_eq!(report.retained, 0);
        assert!(state.operational.hyper_edges.is_empty());
        assert_eq!(state.operational.outer_candidates.len(), 1);
        assert_eq!(state.semantic.relation_count(), 0);
    }

    #[test]
    fn drain_moves_calculus_and_congruence_edges() {
        let mut store = TermStore::new();
        let e = push_symbol(&mut store, "e");
        let v = push_symbol(&mut store, "v");
        let r = push_symbol(&mut store, "r");
        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![e, v, r],
            predicate: predicates::DERIVATIVE_OF,
        });
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![e, v, r],
            predicate: predicates::CONGRUENCE,
        });
        let report = drain_hyper_edges_to_outer_pool(&store, &mut state);
        assert_eq!(report.staged, 2);
        assert_eq!(report.retained, 0);
        assert_eq!(state.operational.outer_candidates.len(), 2);
        assert_eq!(state.semantic.relation_count(), 0);
    }
}
