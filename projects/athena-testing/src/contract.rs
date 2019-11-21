//! Living `12` / `27` contract smoke — typed construction and evaluation only.
//!
//! These lib tests seed the neutral acceptance suite. Product crates still own
//! large integration coverage under `tests/main.rs`. This package must never
//! grow a `tests/` integration binary or dialect fixtures.

use athena_engine::api::AthenaRequest;
use athena_engine::api::request::SessionCommand;
use athena_engine::reasoning::trs::TermPattern;
use athena_engine::runtime::values::arena::push_extension;
use athena_ir::{ApplicationHead, Atom, MathematicalConstant, SemanticOperator, TermNode, UnaryFunction};
use athena_types::CollectionKind;

use crate::{assert_exact_integer, SessionFixture};

#[test]
fn math_constant_pi_is_typed_atom_not_user_symbol() {
    let mut fx = SessionFixture::new();
    let pi = {
        let mut t = fx.terms();
        t.math_constant(MathematicalConstant::Pi)
    };
    let named = {
        let mut t = fx.terms();
        t.symbol("Pi")
    };
    assert!(!fx.structural_eq(pi, named));
    match fx.session().arena.get(pi) {
        Some(TermNode::Atom(Atom::Constant(MathematicalConstant::Pi))) => {}
        other => panic!("expected typed Pi constant, got {other:?}"),
    }
    match fx.session().arena.get(named) {
        Some(TermNode::Atom(Atom::Symbol(_))) => {}
        other => panic!("expected user symbol Pi, got {other:?}"),
    }
}

#[test]
fn ordered_collection_carries_explicit_kind() {
    let mut fx = SessionFixture::new();
    let one = {
        let mut t = fx.terms();
        t.integer(1)
    };
    let two = {
        let mut t = fx.terms();
        t.integer(2)
    };
    let list = {
        let mut t = fx.terms();
        t.ordered([one, two])
    };
    match fx.session().arena.get(list) {
        Some(TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements,
        }) if elements.len() == 2 => {}
        other => panic!("expected OrderedCollection of length 2, got {other:?}"),
    }
}

#[test]
fn semantic_add_and_cos_pi_evaluate_exactly() {
    let mut fx = SessionFixture::new();
    let sum = {
        let mut t = fx.terms();
        let a = t.integer(2);
        let b = t.integer(3);
        t.add([a, b])
    };
    let sum_out = fx.evaluate_term(sum);
    assert_exact_integer(fx.session(), sum_out, 5);

    let cos_pi = {
        let mut t = fx.terms();
        let pi = t.math_constant(MathematicalConstant::Pi);
        t.unary_function(UnaryFunction::Cos, pi)
    };
    let cos_out = fx.evaluate_term(cos_pi);
    assert_exact_integer(fx.session(), cos_out, -1);
}

#[test]
fn register_compiled_rule_dispatches_extension_apply() {
    let mut fx = SessionFixture::new();
    let x_sym = {
        let mut t = fx.terms();
        t.intern("x")
    };
    let f_op = fx.session_mut().operators.intern("f");
    let table = fx.session_mut().defs.alloc_dispatch_table();
    fx.session_mut().defs.bind_operator_table(f_op, table);
    let rhs = {
        let mut t = fx.terms();
        let x = t.symbol("x");
        let two = t.integer(2);
        t.power(x, two)
    };
    let pattern = TermPattern::Application {
        operator: ApplicationHead::Extension(f_op),
        arguments: vec![TermPattern::Bind {
            name: x_sym,
            inner: Box::new(TermPattern::Any),
        }],
    };
    let rule = fx.session_mut().compiled_rules.intern(pattern, rhs);
    fx.execute_request(AthenaRequest::Command(SessionCommand::RegisterRuleDispatch { table, rule }))
        .expect("register compiled rule");

    let call = {
        let three = {
            let mut t = fx.terms();
            t.integer(3)
        };
        push_extension(fx.session_mut(), f_op, vec![three])
    };
    let out = fx.evaluate_term(call);
    assert_exact_integer(fx.session(), out, 9);
}

#[test]
fn zero_ary_sin_maps_over_ordered_collection() {
    let mut fx = SessionFixture::new();
    let mapped = {
        let mut t = fx.terms();
        let sin = t.semantic(SemanticOperator::from_unary(UnaryFunction::Sin), []);
        let zero = t.integer(0);
        let list = t.ordered([zero]);
        t.semantic(SemanticOperator::Map, [sin, list])
    };
    let out = fx.evaluate_term(mapped);
    match fx.session().arena.get(out) {
        Some(TermNode::Collection {
            kind: CollectionKind::OrderedCollection,
            elements,
        }) if elements.len() == 1 => {
            assert_exact_integer(fx.session(), elements[0], 0);
        }
        other => panic!("expected OrderedCollection[0], got {other:?}"),
    }
}
