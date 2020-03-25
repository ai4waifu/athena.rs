//! 自 `src/reasoning/mgraph/core/predicate_registry.rs` 迁出的原内联测试。

use athena_engine::reasoning::mgraph::{PredicateId, arity_ok, descriptor, predicates};

#[test]
fn every_builtin_predicate_has_descriptor() {
    for id in [
        predicates::POLYNOMIAL_RESULT,
        predicates::CONGRUENCE,
        predicates::REWRITE_EQUIVALENT,
        predicates::EVALUATION_RESULT,
        predicates::DERIVATIVE_OF,
        predicates::SERIES_EXPANSION,
        predicates::INTEGRAL_OF,
    ] {
        assert!(descriptor(id).is_some(), "missing descriptor for {id:?}");
        assert!(arity_ok(id, *descriptor(id).unwrap().subject_arity.start()));
    }
}

#[test]
fn unknown_predicate_has_no_descriptor() {
    assert!(descriptor(PredicateId(0)).is_none());
    assert!(descriptor(PredicateId(99)).is_none());
    assert!(!arity_ok(PredicateId(99), 2));
}
