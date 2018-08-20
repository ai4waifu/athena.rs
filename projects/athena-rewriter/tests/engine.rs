use athena_ir::{AtomKind, TermArena, TermBuilder, TermKind};
use athena_rewriter::Rewriter;
use athena_types::{Number, SourceSpan};

#[test]
fn simplify_noop_on_atom() {
    let mut arena = TermArena::new();
    let mut b = TermBuilder::new(&mut arena);
    let n = b.number(Number::small_int(2), SourceSpan::default());
    let r = Rewriter::new().simplify(&mut arena, n).unwrap();
    assert!(!r.changed);
    assert_eq!(arena.get(n), Some(&TermKind::Atom(AtomKind::Number(Number::small_int(2)))));
}
