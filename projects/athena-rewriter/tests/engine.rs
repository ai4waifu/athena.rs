use athena_ir::{Atom, ExprArena, ExprBuilder, ExprNode, numeric::NumericValue};
use athena_rewriter::Rewriter;
use athena_types::SourceSpan;

#[test]
fn simplify_noop_on_atom() {
    let mut arena = ExprArena::new();
    let mut b = ExprBuilder::new(&mut arena);
    let n = b.number(NumericValue::small_int(2), SourceSpan::default());
    let r = Rewriter::new().simplify(&mut arena, n).unwrap();
    assert!(!r.changed);
    assert_eq!(arena.get(n), Some(&ExprNode::Atom(Atom::Number(NumericValue::small_int(2)))));
}
