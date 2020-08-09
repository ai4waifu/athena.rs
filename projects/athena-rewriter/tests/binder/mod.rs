//! `athena-rewriter` 绑定器合同。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode, TermStore};
use athena_rewriter::{TermPattern, match_pattern, substitute};
use athena_types::{CollectionKind, SourceSpan};

#[test]
fn bind_is_consistent_across_repeated_names() {
    let mut store = TermStore::new();
    let span = SourceSpan::default();
    let one = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))), span);
    let two = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(2))), span);
    let x = store.symbols_mut().intern("x");
    let same = store.push(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Add), arguments: vec![one, one] }, span);
    let diff = store.push(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Add), arguments: vec![one, two] }, span);
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Semantic(SemanticOperator::Add),
        arguments: vec![
            TermPattern::Bind { name: x, inner: Box::new(TermPattern::Any) },
            TermPattern::Bind { name: x, inner: Box::new(TermPattern::Any) },
        ],
    };
    let mut binds = athena_rewriter::PatternBindings::new();
    assert!(match_pattern(&store, same, &pattern, &mut binds));
    assert_eq!(binds.get(&x), Some(&one));

    let mut binds = athena_rewriter::PatternBindings::new();
    assert!(!match_pattern(&store, diff, &pattern, &mut binds));
}

#[test]
fn sequence_matches_ordered_collection() {
    let mut store = TermStore::new();
    let span = SourceSpan::default();
    let a = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))), span);
    let b = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(2))), span);
    let list = store.push(TermNode::Collection { kind: CollectionKind::OrderedCollection, elements: vec![a, b] }, span);
    let pattern = TermPattern::Sequence(vec![TermPattern::Exact(a), TermPattern::Exact(b)]);
    let mut binds = athena_rewriter::PatternBindings::new();
    assert!(match_pattern(&store, list, &pattern, &mut binds));
}

#[test]
fn substitute_replaces_bound_symbols_and_hash_conses() {
    let mut store = TermStore::new();
    let span = SourceSpan::default();
    let x = store.symbols_mut().intern("x");
    let y = store.symbols_mut().intern("y");
    let x_term = store.push(TermNode::Atom(Atom::Symbol(x)), span);
    let y_term = store.push(TermNode::Atom(Atom::Symbol(y)), span);
    let one = store.push(TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))), span);
    let template =
        store.push(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Add), arguments: vec![x_term, y_term] }, span);
    let mut binds = athena_rewriter::PatternBindings::new();
    binds.insert(x, one);
    binds.insert(y, one);
    let out = substitute(&mut store, template, &binds);
    let expected =
        store.push(TermNode::Application { head: ApplicationHead::Semantic(SemanticOperator::Add), arguments: vec![one, one] }, span);
    assert_eq!(out, expected);
}
