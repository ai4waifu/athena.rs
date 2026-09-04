use athena_ir::{Atom, ExprArena, ExprNode};
use athena_numeric::NumericValue;
use athena_types::SourceSpan;

#[test]
fn push_and_verify() {
    let mut arena = ExprArena::new();
    let n = arena.push(ExprNode::Atom(Atom::Number(NumericValue::small_int(1))), SourceSpan::default());
    arena.verify(n).unwrap();
}
