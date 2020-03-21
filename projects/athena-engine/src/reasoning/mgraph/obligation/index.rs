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
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct ReflectorWake {
    /// Obligation that can now re-reflect.
    pub obligation: ProofObligation,
    /// Newly admitted relation that matched.
    pub relation: RelationRef,
}

impl ReflectorWake {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            obligation: self.obligation.owning_copy(),
            relation: self.relation,
        }
    }
}

/// Report from draining wakes for one admit.
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct WakeReport {
    /// Obligations removed from the pending index and handed to the caller.
    pub wakes: Vec<ReflectorWake>,
}

impl WakeReport {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            wakes: self.wakes.iter().map(ReflectorWake::owning_copy).collect(),
        }
    }
}

/// Pending obligations keyed for predicate / scope wake matching.
///
/// Living `31`：**不**实现 [`Clone`]。
#[derive(Debug, Default, PartialEq, Eq)]
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
    /// An obligation matches when predicates equal, scopes are not
    /// `IncompatibleWith`, and the obligation can see the admitted fiber via
    /// identity / `Refines` ancestors / directed `CompatibleWith`.
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
                && !scopes.incompatible_with(obligation.scope, admitted_scope)
                && (scopes.is_refines_ancestor(obligation.scope, admitted_scope) || scopes.compatible_with(obligation.scope, admitted_scope));
            if visible {
                wakes.push(ReflectorWake { obligation, relation });
            }
            else {
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
    use crate::reasoning::mgraph::{ScopeRelationKind, core::refs::predicates, facts::FactId};

    #[test]
    fn wake_removes_matching_obligation() {
        let mut index = ObligationIndex::new();
        index.register(ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] });
        let scopes = ScopeIndex::new();
        let report = index.wake_matching(ScopeRef::UNCONDITIONAL, predicates::POLYNOMIAL_RESULT, FactId(0), &scopes);
        assert_eq!(report.wakes.len(), 1);
        assert!(index.is_empty());
    }

    #[test]
    fn wake_respects_refines_visibility() {
        let mut index = ObligationIndex::new();
        let local = ScopeRef(3);
        index.register(ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: local, known_objects: vec![] });
        let mut scopes = ScopeIndex::new();
        scopes.try_add_relation(local, ScopeRef::UNCONDITIONAL, ScopeRelationKind::Refines).expect("refines");

        let miss = index.wake_matching(ScopeRef::UNCONDITIONAL, predicates::CONGRUENCE, FactId(1), &scopes);
        assert!(miss.wakes.is_empty());
        assert_eq!(index.len(), 1);

        let hit = index.wake_matching(ScopeRef::UNCONDITIONAL, predicates::POLYNOMIAL_RESULT, FactId(2), &scopes);
        assert_eq!(hit.wakes.len(), 1);
        assert_eq!(hit.wakes[0].relation, FactId(2));
        assert!(index.is_empty());
    }

    #[test]
    fn finer_admit_does_not_wake_coarser_obligation() {
        let mut index = ObligationIndex::new();
        index.register(ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: ScopeRef::UNCONDITIONAL, known_objects: vec![] });
        let mut scopes = ScopeIndex::new();
        let local = ScopeRef(4);
        scopes.try_add_relation(local, ScopeRef::UNCONDITIONAL, ScopeRelationKind::Refines).expect("refines");
        let report = index.wake_matching(local, predicates::POLYNOMIAL_RESULT, FactId(9), &scopes);
        assert!(report.wakes.is_empty());
        assert_eq!(index.len(), 1);
    }

    #[test]
    fn wake_respects_compatible_and_incompatible() {
        let mut index = ObligationIndex::new();
        let a = ScopeRef(5);
        let b = ScopeRef(6);
        index.register(ProofObligation { predicate: predicates::POLYNOMIAL_RESULT, scope: a, known_objects: vec![] });
        let mut scopes = ScopeIndex::new();
        scopes.try_add_relation(a, b, ScopeRelationKind::IncompatibleWith).expect("incompatible");

        let blocked = index.wake_matching(b, predicates::POLYNOMIAL_RESULT, FactId(9), &scopes);
        assert!(blocked.wakes.is_empty());
        assert_eq!(index.len(), 1);

        let mut scopes2 = ScopeIndex::new();
        scopes2.try_add_relation(a, b, ScopeRelationKind::CompatibleWith).expect("compatible");
        let hit = index.wake_matching(b, predicates::POLYNOMIAL_RESULT, FactId(10), &scopes2);
        assert_eq!(hit.wakes.len(), 1);
    }
}
