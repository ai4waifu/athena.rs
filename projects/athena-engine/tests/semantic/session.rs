//! Session 语义表冒烟测试。

use athena_engine::{
    AssumptionScope, AssumptionScopeTable, ExprBindingTable, Predicate, ResultIdTable, Session, SymbolId, TermId, ValueIdTable,
};
use athena_gc::{EmptyObjectGraph, GcMode, RootKind};

#[test]
fn session_owns_sem0_sem1_tables() {
    let session = Session::new();
    assert!(session.exprs.is_empty());
    assert!(session.values.is_empty());
    assert!(session.results.is_empty());
    assert!(session.assumption_scopes.is_empty());
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
fn expr_binding_separates_term_storage_from_expr_identity() {
    let mut exprs = ExprBindingTable::new();
    let term = TermId(42);
    let e1 = exprs.intern_term(term);
    let e2 = exprs.intern_term(term);
    assert_eq!(e1, e2);
    assert_eq!(exprs.term_of(e1), Some(term));
    assert_eq!(exprs.expr_of(term), Some(e1));
    // 同数值载荷的 TermId/ExprId 不自动等同：另一存储槽拿到新 ExprId。
    let other = exprs.intern_term(TermId(7));
    assert_ne!(other, e1);
    assert_eq!(other.0, 1);
    assert_eq!(e1.0, 0);
}

#[test]
fn value_and_result_ids_allocate_independently() {
    let mut values = ValueIdTable::new();
    let mut results = ResultIdTable::new();
    let v0 = values.alloc();
    let r0 = results.alloc();
    assert_eq!(v0.0, 0);
    assert_eq!(r0.0, 0);
    assert!(values.contains(v0));
    assert!(results.contains(r0));
    assert!(!values.contains(athena_types::ValueId(99)));
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
    let bad = AssumptionScope::from_predicates(vec![
        Predicate::Equal(TermId(1), TermId(2)),
        Predicate::NotEqual(TermId(1), TermId(2)),
    ]);
    let err = table.intern(bad).expect_err("conflict");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("local_conflict"));
}
