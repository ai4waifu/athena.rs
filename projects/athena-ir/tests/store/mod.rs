use athena_ir::{Atom, TermNode, TermStore};
use athena_numeric::NumericValue;
use athena_types::SourceSpan;

#[test]
fn push_and_verify() {
    let mut arena = TermStore::new();
    let n = arena.push(TermNode::Atom(Atom::Number(NumericValue::small_int(1))), SourceSpan::default());
    arena.verify(n).unwrap();
}
