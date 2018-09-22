use athena_ir::{AtomKind, TermArena, TermKind};
use athena_numeric::NumericValue;
use athena_types::SourceSpan;

#[test]
fn push_and_verify() {
    let mut arena = TermArena::new();
    let n = arena.push(TermKind::Atom(AtomKind::Number(NumericValue::small_int(1))), SourceSpan::default());
    arena.verify(n).unwrap();
}
