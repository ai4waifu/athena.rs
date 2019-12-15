use athena_ir::{Atom, TermNode, TermStore};
use athena_numeric::NumericValue;
use athena_types::SourceSpan;

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
    let add1 = arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![x, y],
        },
        span,
    );
    let add2 = arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![x, y],
        },
        span,
    );
    assert_eq!(add1, add2);
    assert_eq!(arena.len(), 3);
}
