//! 自 `src/reasoning/mgraph/relations/scope.rs` 迁出的原内联测试。

use athena_engine::reasoning::mgraph::{
    core::refs::{ScopeRef, ScopeRelationKind},
    relations::{ScopeIndex, ScopeRelationConflict},
};
use athena_types::{Diagnostic, DiagnosticCode};

#[test]
fn rejects_compatible_then_incompatible() {
    let mut scopes = ScopeIndex::new();
    let a = ScopeRef(1);
    let b = ScopeRef(2);
    scopes.try_add_relation(a, b, ScopeRelationKind::CompatibleWith).expect("compatible");
    let err = scopes.try_add_relation(a, b, ScopeRelationKind::IncompatibleWith).expect_err("conflict");
    assert_eq!(err, ScopeRelationConflict::CompatibleAndIncompatible { a, b });
    assert!(!scopes.incompatible_with(a, b));
    assert_eq!(err.into_diagnostic().details.get("reason").map(|v| v.to_string()).as_deref(), Some("compatible_and_incompatible"));
}

#[test]
fn rejects_incompatible_then_compatible() {
    let mut scopes = ScopeIndex::new();
    let a = ScopeRef(3);
    let b = ScopeRef(4);
    scopes.try_add_relation(a, b, ScopeRelationKind::IncompatibleWith).expect("incompatible");
    let err = scopes.try_add_relation(a, b, ScopeRelationKind::CompatibleWith).expect_err("conflict");
    assert!(matches!(err, ScopeRelationConflict::CompatibleAndIncompatible { .. }));
    assert!(!scopes.compatible_with(a, b));
}

#[test]
fn rejects_refines_cycle() {
    let mut scopes = ScopeIndex::new();
    let a = ScopeRef(5);
    let b = ScopeRef(6);
    scopes.try_add_relation(a, b, ScopeRelationKind::Refines).expect("a ⊑ b");
    let err = scopes.try_add_relation(b, a, ScopeRelationKind::Refines).expect_err("cycle");
    assert_eq!(err, ScopeRelationConflict::RefinesWouldCycle { from: b, to: a });
}

#[test]
fn rejects_self_incompatible() {
    let s = ScopeRef(7);
    let mut scopes = ScopeIndex::new();
    let err = scopes.try_add_relation(s, s, ScopeRelationKind::IncompatibleWith).expect_err("self");
    assert_eq!(err, ScopeRelationConflict::SelfIncompatible { scope: s });
}
