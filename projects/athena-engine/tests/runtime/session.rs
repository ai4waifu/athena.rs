//! Session 语义表冒烟测试。

use athena_engine::runtime::{
    ComputationResult, CoverageStatus, ResultStore, RuntimeValue, Session, ValueStore, semantic::AssumptionScopeTable,
};
use athena_gc::{EmptyObjectGraph, GcMode, RootKind};
use athena_types::{AssumptionScope, ComputationStatus, Predicate, SymbolId, TermId};

#[test]
fn session_owns_sem0_sem1_tables() {
    let session = Session::new();
    assert!(session.values.is_empty());
    assert!(session.results.is_empty());
    assert!(session.assumption_scopes.is_empty());
    assert_eq!(session.arena.len(), 0);
}

#[test]
fn session_default_heap_is_deferred() {
    let session = Session::new();
    assert_eq!(session.heap().borrow().effective_mode(), GcMode::Deferred);
    assert_eq!(session.numeric_context().heap().borrow().id(), session.heap().borrow().id());
}

#[test]
fn session_heap_roots_keep_object_across_collect() {
    let session = Session::new();
    assert_eq!(session.heap().borrow().effective_mode(), GcMode::Deferred);
    let obj = session.heap().borrow_mut().allocate_object(8).expect("obj");
    session.heap().borrow_mut().object_payload_mut(obj).expect("w")[0] = 9;
    let token = session.register_root(obj, RootKind::Session);
    let report = session.collect().expect("collect");
    assert_eq!(report.objects_swept, 0);
    assert_eq!(session.heap().borrow().object_payload(obj).expect("ro")[0], 9);
    assert!(session.unregister_root(token));
    let report = session.heap().borrow_mut().collect_traced(&EmptyObjectGraph).expect("collect2");
    assert!(report.objects_swept >= 1);
}

#[test]
fn term_id_is_native_storage_identity() {
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    assert_ne!(a, b);
    assert_eq!(session.arena.get(a).is_some(), true);
    assert_eq!(session.arena.len(), 2);
}

#[test]
fn value_store_owns_runtime_payload_not_term_bijection() {
    let mut session = Session::new();
    let term = session.builder().int(7, Default::default());
    let v0 = session.insert_symbolic_value(term);
    let v1 = session.insert_value(RuntimeValue::Boolean(true));
    assert_eq!(session.symbolic_term_of_value(v0), Some(term));
    assert_eq!(session.symbolic_term_of_value(v1), None);
    // Same term inserted twice yields distinct ValueIds (no term↔value bijection).
    let v2 = session.insert_symbolic_value(term);
    assert_ne!(v0, v2);
    assert_eq!(session.values.count(), 3);
}

#[test]
fn result_store_owns_computation_result_payload() {
    let mut values = ValueStore::new();
    let mut results = ResultStore::new();
    let value = values.insert(RuntimeValue::Null);
    let result = ComputationResult::with_status(ComputationStatus::Invalid, CoverageStatus::Unsupported)
        .with_value(value)
        .with_diagnostic(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation));
    let r0 = results.insert(result);
    assert!(results.contains(r0));
    let loaded = results.get(r0).expect("payload");
    assert_eq!(loaded.status, ComputationStatus::Invalid);
    assert_eq!(loaded.coverage, CoverageStatus::Unsupported);
    assert_eq!(loaded.value, Some(value));
    assert_eq!(loaded.diagnostics.len(), 1);
}

#[test]
fn assumption_scope_table_intern_inherit_and_merge() {
    let mut table = AssumptionScopeTable::new();
    let parent = table.intern(AssumptionScope::from_predicates(vec![Predicate::SymbolReal(SymbolId(0))])).unwrap();
    let child = table.intern(AssumptionScope::inherit(parent, vec![Predicate::SymbolNonZero(SymbolId(0))])).unwrap();
    let expanded = table.inherited_predicates(child).unwrap();
    assert_eq!(expanded.len(), 2);

    let other = table.intern(AssumptionScope::from_predicates(vec![Predicate::SymbolReal(SymbolId(1))])).unwrap();
    let merged = table.merge_interned(parent, other).unwrap();
    let merged_scope = table.get(merged).unwrap();
    assert_eq!(merged_scope.predicates.len(), 2);
}

#[test]
fn assumption_scope_table_rejects_local_conflict() {
    let mut table = AssumptionScopeTable::new();
    let bad = AssumptionScope::from_predicates(vec![Predicate::Equal(TermId(1), TermId(2)), Predicate::NotEqual(TermId(1), TermId(2))]);
    let err = table.intern(bad).expect_err("conflict");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("local_conflict"));
}
