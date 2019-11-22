use athena_ir::{Atom, TermBuilder, TermNode, TermStore, numeric::NumericValue};
use athena_rewriter::{Rewriter, RuleSet};
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

#[test]
fn rule_set_registers_typed_rewrite_ids() {
    let mut arena = TermStore::new();
    let mut b = TermBuilder::new(&mut arena);
    let pattern = b.number(NumericValue::small_int(0), SourceSpan::default());
    let replacement = b.number(NumericValue::small_int(1), SourceSpan::default());
    let mut rules = RuleSet::new();
    let id = rules.push(pattern, replacement, Some("smoke"));
    assert_eq!(rules.len(), 1);
    assert_eq!(rules.get(id).map(|r| r.pattern), Some(pattern));
}
