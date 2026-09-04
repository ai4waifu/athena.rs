//! 身份分离与 `AssumptionScope` 合同。

use athena_types::{
    AssumptionBranchPolicy, AssumptionScope, AssumptionScopeId, AssumptionSet, Predicate, ProofRef, ResultId,
    ScopeApplicability, ScopeConflictKind, ScopeMergeOutcome, SymbolId, TermId, TheoryContext, TheoryContextId, ValueId,
};

#[test]
fn sem0_ids_are_distinct_newtypes() {
    let e = TermId(1);
    let v = ValueId(1);
    let r = ResultId(1);
    let p = ProofRef(1);
    // 同数值载荷不代表同一语义身份；类型系统禁止直接互转。
    assert_eq!(e.0, v.0);
    assert_eq!(r.0, e.0);
    assert_eq!(p.0, 1);
    let _applicability = ScopeApplicability::Conditional { scope: AssumptionScopeId(0) };
    assert!(matches!(_applicability, ScopeApplicability::Conditional { .. }));
}

#[test]
fn assumption_scope_merge_detects_equal_vs_not_equal() {
    let a = AssumptionScope::from_predicates(vec![Predicate::Equal(TermId(1), TermId(2))]);
    let b = AssumptionScope::from_predicates(vec![Predicate::NotEqual(TermId(1), TermId(2))]);
    match a.merge(&b) {
        ScopeMergeOutcome::Conflict(c) => assert_eq!(c.kind, ScopeConflictKind::PredicateContradiction),
        ScopeMergeOutcome::Ok(_) => panic!("expected conflict"),
    }
}

#[test]
fn assumption_scope_merge_unions_compatible_predicates() {
    let a = AssumptionScope::from_predicates(vec![Predicate::SymbolReal(SymbolId(0))]);
    let b = AssumptionScope::from_predicates(vec![Predicate::SymbolNonZero(SymbolId(0))]);
    match a.merge(&b) {
        ScopeMergeOutcome::Ok(m) => {
            assert_eq!(m.predicates.len(), 2);
            assert!(m.local_conflict().is_none());
        }
        ScopeMergeOutcome::Conflict(c) => panic!("unexpected {c:?}"),
    }
}

#[test]
fn assumption_scope_inherit_and_expand() {
    let parent_id = AssumptionScopeId(7);
    let parent = AssumptionScope {
        id: Some(parent_id),
        predicates: vec![Predicate::SymbolReal(SymbolId(1))],
        ..AssumptionScope::unconditional()
    };
    let child = AssumptionScope::inherit(parent_id, vec![Predicate::SymbolNonZero(SymbolId(1))]);
    let expanded = child.inherited_predicates(|id| if id == parent_id { Some(parent.clone()) } else { None });
    assert_eq!(expanded.len(), 2);
    assert_eq!(expanded[0], Predicate::SymbolReal(SymbolId(1)));
    assert_eq!(expanded[1], Predicate::SymbolNonZero(SymbolId(1)));
}

#[test]
fn assumption_scope_project_keeps_symbol_predicates() {
    let scope = AssumptionScope::from_predicates(vec![
        Predicate::SymbolReal(SymbolId(0)),
        Predicate::SymbolNonZero(SymbolId(1)),
        Predicate::Equal(TermId(3), TermId(4)),
    ]);
    let projected = scope.project_to_symbols(&[SymbolId(0)]);
    assert_eq!(projected.predicates, vec![Predicate::SymbolReal(SymbolId(0))]);
}

#[test]
fn theory_context_conflict_and_grh_rh_compat() {
    let grh = AssumptionScope { theory_context: TheoryContext::UnderGRH, ..AssumptionScope::unconditional() };
    let rh = AssumptionScope { theory_context: TheoryContext::UnderRH, ..AssumptionScope::unconditional() };
    match grh.merge(&rh) {
        ScopeMergeOutcome::Ok(m) => assert_eq!(m.theory_context, TheoryContext::UnderGRH),
        ScopeMergeOutcome::Conflict(c) => panic!("unexpected {c:?}"),
    }
    let sch = AssumptionScope { theory_context: TheoryContext::UnderSchanuel, ..AssumptionScope::unconditional() };
    let abc = AssumptionScope { theory_context: TheoryContext::UnderGeneralizedABC, ..AssumptionScope::unconditional() };
    match sch.merge(&abc) {
        ScopeMergeOutcome::Conflict(c) => assert_eq!(c.kind, ScopeConflictKind::TheoryContextMismatch),
        ScopeMergeOutcome::Ok(_) => panic!("expected theory conflict"),
    }
    let _ = TheoryContextId(0);
    let _ = AssumptionBranchPolicy::Principal;
}

#[test]
fn assumption_set_roundtrip_lift() {
    let set = AssumptionSet::from_predicates(vec![Predicate::SymbolReal(SymbolId(2))]);
    let scope = AssumptionScope::from_assumption_set(&set);
    let back = scope.to_assumption_set();
    assert_eq!(back.predicates, set.predicates);
}
