//! Numeric evidence arena tests.

use athena_numeric::{Integer, NumericBinding, NumericEvidenceArena, NumericEvidenceRecord, NumericValue};

#[test]
fn arena_intern_tags_deduplicates() {
    let mut arena = NumericEvidenceArena::new();
    let a = arena.intern_tags(vec!["source".into(), "fold".into()]);
    let b = arena.intern_tags(vec!["source".into(), "fold".into()]);
    assert_eq!(a, b);
    assert_eq!(arena.len(), 1);
    assert_eq!(arena.resolve(a).unwrap().tags, vec!["source", "fold"]);
}

#[test]
fn binding_equality_ignores_evidence_id() {
    let mut arena = NumericEvidenceArena::new();
    let v = NumericValue::integer(Integer::from_i64(3));
    let e1 = arena.intern_tags(vec!["a".into()]);
    let e2 = arena.allocate(NumericEvidenceRecord { tags: vec!["b".into()], certificate: None });
    let b1 = NumericBinding::with_evidence(v.clone(), e1);
    let b2 = NumericBinding::with_evidence(v, e2);
    assert_eq!(b1, b2);
    assert_ne!(b1.evidence(), b2.evidence());
}

#[test]
fn value_clone_does_not_duplicate_arena_payload() {
    let mut arena = NumericEvidenceArena::new();
    let id = arena.intern_tags(vec!["witness".into()]);
    let binding = NumericBinding::with_evidence(NumericValue::small_int(1), id);
    let cloned = binding.value().clone();
    assert_eq!(cloned, NumericValue::small_int(1));
    assert_eq!(arena.len(), 1);
}
