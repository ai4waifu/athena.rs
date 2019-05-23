use athena_ir::{Atom, TermBuilder, TermNode, TermStore, numeric::NumericValue};
use athena_rewriter::Rewriter;
use athena_types::SourceSpan;

#[test]
fn simplify_noop_on_atom() {
    let mut arena = TermStore::new();
    let mut b = TermBuilder::new(&mut arena);
    let n = b.number(NumericValue::small_int(2), SourceSpan::default());
    let r = Rewriter::new().simplify(&mut arena, n).unwrap();
    assert!(!r.changed);
    assert_eq!(arena.get(n), Some(&TermNode::Atom(Atom::Number(NumericValue::small_int(2)))));
}
