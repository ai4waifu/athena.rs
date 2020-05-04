use athena_ir::{Atom, TermNode, TermStore};
use athena_numeric::NumericValue;
use athena_types::{SourceSpan, TermRef};

#[test]
fn push_and_verify() {
    let mut arena = TermStore::new();
    let n = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(1))), SourceSpan::default());
    arena.verify(n).unwrap();
}

#[test]
fn push_hash_conses_identical_atoms() {
    let mut arena = TermStore::new();
    let span = SourceSpan::default();
    let a = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(5))), span);
    let b = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(5))), span);
    let c = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(6))), span);
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(arena.len(), 2);
}

#[test]
fn push_hash_conses_applications_with_shared_children() {
    use athena_ir::{ApplicationHead, SemanticOperator};

    let mut arena = TermStore::new();
    let span = SourceSpan::default();
    let x = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(1))), span);
    let y = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(2))), span);
    let add1 = arena.push(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Add), arguments: vec![x, y] }, span);
    let add2 = arena.push(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Add), arguments: vec![x, y] }, span);
    assert_eq!(add1, add2);
    assert_eq!(arena.len(), 3);
}

#[test]
fn term_ref_tracks_store_epoch() {
    let mut arena = TermStore::new();
    assert_eq!(arena.epoch(), 1);
    let id = arena.push(TermNode::Atom(Atom::Boolean(true)), SourceSpan::default());
    let live = arena.term_ref(id).expect("ref");
    assert_eq!(live, TermRef::new(id, 1));
    assert_eq!(arena.check_ref(live).expect("live"), id);

    arena.bump_epoch();
    assert_eq!(arena.epoch(), 2);
    let err = arena.check_ref(live).expect_err("stale");
    assert_eq!(
        err.details.get("reason").map(|v| v.to_string()).as_deref(),
        Some("stale_term_generation")
    );
    let refreshed = arena.term_ref(id).expect("ref2");
    assert_eq!(arena.check_ref(refreshed).expect("ok"), id);
}
