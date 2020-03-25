//! 自 `src/reasoning/mgraph/obligation/index.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    reasoning::mgraph::{ScopeIndex, ScopeRef, ScopeRelationKind, core::refs::predicates, facts::FactId, obligation::*},
};

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
