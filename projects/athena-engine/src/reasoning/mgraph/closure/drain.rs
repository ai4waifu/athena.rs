//! Drain operational hyper-edges into the OuterCandidate pool (Living `26`).

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
pub fn drain_hyper_edges_to_outer_pool(state: &mut MGraphState) -> HyperEdgeDrainReport {
    drain_operational_hyper_edges(&mut state.operational)
}

fn drain_operational_hyper_edges(operational: &mut OperationalState) -> HyperEdgeDrainReport {
    let pending = std::mem::take(&mut operational.hyper_edges);
    let mut retained = Vec::new();
    let mut staged = 0u32;
    for edge in pending {
        match hyper_edge_to_outer_candidate(&edge) {
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
    use athena_types::TermId;

    use crate::reasoning::mgraph::{HyperEdge, MGraphState, predicates};

    use super::*;

    #[test]
    fn drain_moves_rewrite_edges_without_admitting() {
        let mut state = MGraphState::new();
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![TermId(1), TermId(2)],
            predicate: predicates::REWRITE_EQUIVALENT,
        });
        state.operational.hyper_edges.push(HyperEdge {
            nodes: vec![TermId(3), TermId(4), TermId(5)],
            predicate: predicates::CONGRUENCE,
        });
        let report = drain_hyper_edges_to_outer_pool(&mut state);
        assert_eq!(report.staged, 1);
        assert_eq!(report.retained, 1);
        assert_eq!(state.operational.outer_candidates.len(), 1);
        assert_eq!(state.operational.hyper_edges.len(), 1);
        assert_eq!(state.semantic.derived.exact_uf.union_count(), 0);
        assert_eq!(state.semantic.relation_count(), 0);
    }
}
