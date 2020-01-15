//! Obligation index and Reflector wake queue (Living `29` · bootstrap).
//!
//! Pending [`ProofObligation`]s live in operational state. Admission may wake
//! matching obligations; waking does **not** write SemanticCore.

use crate::reasoning::mgraph::{
    core::refs::{PredicateId, RelationRef, ScopeRef},
    obligation::ProofObligation,
    relations::scope::ScopeIndex,
};

/// One Reflector wake produced after an admit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectorWake {
    /// Obligation that can now re-reflect.
    pub obligation: ProofObligation,
    /// Newly admitted relation that matched.
    pub relation: RelationRef,
}

/// Report from draining wakes for one admit.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WakeReport {
    /// Obligations removed from the pending index and handed to the caller.
    pub wakes: Vec<ReflectorWake>,
}

/// Pending obligations keyed for predicate / scope wake matching.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObligationIndex {
    pending: Vec<ProofObligation>,
}

impl ObligationIndex {
    /// Empty index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a pending semantic gap.
    pub fn register(&mut self, obligation: ProofObligation) {
        self.pending.push(obligation);
    }

    /// Number of pending obligations.
    pub fn len(&self) -> usize {
        self.pending.len()
    }

    /// Whether no obligations are pending.
    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }

    /// Wake and remove obligations that can observe `admitted_scope` for `predicate`.
    ///
    /// An obligation matches when predicates equal and the obligation scope can see
    /// the admitted fiber via identity or registered `Refines` ancestors.
    pub fn wake_matching(
        &mut self,
        admitted_scope: ScopeRef,
        predicate: PredicateId,
        relation: RelationRef,
        scopes: &ScopeIndex,
    ) -> WakeReport {
        let mut wakes = Vec::new();
        let mut retained = Vec::new();
        for obligation in self.pending.drain(..) {
            let visible = obligation.predicate == predicate
                && scopes.is_refines_ancestor(obligation.scope, admitted_scope);
            if visible {
                wakes.push(ReflectorWake {
                    obligation,
                    relation,
                });
            } else {
                retained.push(obligation);
            }
        }
        self.pending = retained;
        WakeReport { wakes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoning::mgraph::{
        ScopeRelationKind,
        core::refs::predicates,
        facts::FactId,
    };

    #[test]
    fn wake_removes_matching_obligation() {
        let mut index = ObligationIndex::new();
        index.register(ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![],
        });
        let scopes = ScopeIndex::new();
        let report = index.wake_matching(
            ScopeRef::UNCONDITIONAL,
            predicates::POLYNOMIAL_RESULT,
            FactId(0),
            &scopes,
        );
        assert_eq!(report.wakes.len(), 1);
        assert!(index.is_empty());
    }

    #[test]
    fn wake_respects_refines_visibility() {
        let mut index = ObligationIndex::new();
        let local = ScopeRef(3);
        index.register(ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: local,
            known_objects: vec![],
        });
        let mut scopes = ScopeIndex::new();
        scopes.add_relation(local, ScopeRef::UNCONDITIONAL, ScopeRelationKind::Refines);

        let miss = index.wake_matching(
            ScopeRef::UNCONDITIONAL,
            predicates::CONGRUENCE,
            FactId(1),
            &scopes,
        );
        assert!(miss.wakes.is_empty());
        assert_eq!(index.len(), 1);

        let hit = index.wake_matching(
            ScopeRef::UNCONDITIONAL,
            predicates::POLYNOMIAL_RESULT,
            FactId(2),
            &scopes,
        );
        assert_eq!(hit.wakes.len(), 1);
        assert_eq!(hit.wakes[0].relation, FactId(2));
        assert!(index.is_empty());
    }

    #[test]
    fn finer_admit_does_not_wake_coarser_obligation() {
        let mut index = ObligationIndex::new();
        index.register(ProofObligation {
            predicate: predicates::POLYNOMIAL_RESULT,
            scope: ScopeRef::UNCONDITIONAL,
            known_objects: vec![],
        });
        let mut scopes = ScopeIndex::new();
        let local = ScopeRef(4);
        scopes.add_relation(local, ScopeRef::UNCONDITIONAL, ScopeRelationKind::Refines);
        let report = index.wake_matching(local, predicates::POLYNOMIAL_RESULT, FactId(9), &scopes);
        assert!(report.wakes.is_empty());
        assert_eq!(index.len(), 1);
    }
}
