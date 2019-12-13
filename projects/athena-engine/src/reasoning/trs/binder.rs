//! Store-level [`TermPattern`] binder (Living `27`).
//!
//! Matches against [`TermStore`] only — no Session, no dialect surface names.

use std::collections::HashMap;

use athena_ir::{Atom, TermNode, TermStore};
use athena_types::{SymbolId, TermId, ValueTypeId};

use super::pattern::{PatternConstraint, TermPattern};

/// Binding environment produced by a successful match.
pub type PatternBindings = HashMap<SymbolId, TermId>;

/// Match `pattern` against `expr` in `store`, accumulating bindings.
///
/// [`TermPattern::Bind`] is consistent: a repeated name must refer to structurally
/// equal terms.
pub fn match_pattern(
    store: &TermStore,
    expr: TermId,
    pattern: &TermPattern,
    binds: &mut PatternBindings,
) -> bool {
    match pattern {
        TermPattern::Any => true,
        TermPattern::Bind { name, inner } => {
            if !match_pattern(store, expr, inner, binds) {
                return false;
            }
            match binds.get(name) {
                Some(existing) => store.structural_eq(expr, *existing),
                None => {
                    binds.insert(*name, expr);
                    true
                }
            }
        }
        TermPattern::Exact(literal) => store.structural_eq(expr, *literal),
        TermPattern::Sequence(items) => match store.get(expr) {
            Some(TermNode::Collection { elements, .. }) => zip_match(store, elements, items, binds),
            _ => false,
        },
        TermPattern::Application { operator, arguments } => match store.get(expr) {
            Some(TermNode::Application { head, arguments: args }) => {
                head == operator && zip_match(store, args, arguments, binds)
            }
            _ => false,
        },
        TermPattern::StructuralApplication(items) => match store.get(expr) {
            Some(TermNode::Application { arguments: args, .. }) => zip_match(store, args, items, binds),
            _ => false,
        },
        TermPattern::Constrained { pattern, constraint } => {
            constraint_holds(store, expr, constraint) && match_pattern(store, expr, pattern, binds)
        }
    }
}

fn zip_match(store: &TermStore, exprs: &[TermId], patterns: &[TermPattern], binds: &mut PatternBindings) -> bool {
    if exprs.len() != patterns.len() {
        return false;
    }
    exprs
        .iter()
        .zip(patterns.iter())
        .all(|(e, p)| match_pattern(store, *e, p, binds))
}

fn constraint_holds(store: &TermStore, expr: TermId, constraint: &PatternConstraint) -> bool {
    match constraint {
        PatternConstraint::Operator(expected) => match store.get(expr) {
            Some(TermNode::Application { head, .. }) => head == expected,
            _ => false,
        },
        PatternConstraint::ValueType(ValueTypeId::ExactInteger) => match store.get(expr) {
            Some(TermNode::Atom(Atom::Number(n))) => n.as_exact_integer().is_some() || n.as_integer().is_some(),
            _ => false,
        },
        PatternConstraint::ValueType(ValueTypeId::Symbol) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Symbol(_))))
        }
        PatternConstraint::ValueType(ValueTypeId::String) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::String(_))))
        }
        PatternConstraint::ValueType(ValueTypeId::Boolean) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Boolean(_))))
        }
        PatternConstraint::ValueType(ValueTypeId::Null) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Null)))
        }
        PatternConstraint::ValueType(ValueTypeId::Numeric) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Number(_))))
        }
        PatternConstraint::CollectionKind(kind) => match store.get(expr) {
            Some(TermNode::Collection { kind: k, .. }) => k == kind,
            _ => false,
        },
        PatternConstraint::Domain(_) | PatternConstraint::Predicate(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use athena_ir::{ApplicationHead, SemanticOperator, TermNode};
    use athena_types::{CollectionKind, SourceSpan};

    use super::*;

    #[test]
    fn bind_is_consistent_across_repeated_names() {
        let mut store = TermStore::new();
        let span = SourceSpan::default();
        let one = store.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))),
            span,
        );
        let two = store.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(2))),
            span,
        );
        let x = store.symbols_mut().intern("x");
        let same = store.push(
            TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Add),
                arguments: vec![one, one],
            },
            span,
        );
        let diff = store.push(
            TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Add),
                arguments: vec![one, two],
            },
            span,
        );
        let pattern = TermPattern::Application {
            operator: ApplicationHead::Semantic(SemanticOperator::Add),
            arguments: vec![
                TermPattern::Bind {
                    name: x,
                    inner: Box::new(TermPattern::Any),
                },
                TermPattern::Bind {
                    name: x,
                    inner: Box::new(TermPattern::Any),
                },
            ],
        };
        let mut binds = PatternBindings::new();
        assert!(match_pattern(&store, same, &pattern, &mut binds));
        assert_eq!(binds.get(&x), Some(&one));

        let mut binds = PatternBindings::new();
        assert!(!match_pattern(&store, diff, &pattern, &mut binds));
    }

    #[test]
    fn sequence_matches_ordered_collection() {
        let mut store = TermStore::new();
        let span = SourceSpan::default();
        let a = store.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(1))),
            span,
        );
        let b = store.push(
            TermNode::Atom(Atom::Number(athena_numeric::Number::small_int(2))),
            span,
        );
        let list = store.push(
            TermNode::Collection {
                kind: CollectionKind::OrderedCollection,
                elements: vec![a, b],
            },
            span,
        );
        let pattern = TermPattern::Sequence(vec![TermPattern::Exact(a), TermPattern::Exact(b)]);
        let mut binds = PatternBindings::new();
        assert!(match_pattern(&store, list, &pattern, &mut binds));
    }
}
