//! Neutral builders via `athena-testing` (Living `12` / `27`).

use athena_ir::{ApplicationHead, SemanticOperator, TermNode, UnaryFunction};
use athena_testing::{SessionFixture, assert_exact_integer, assert_structural_eq, term_request};

#[test]
fn add_multiply_constructs_semantic_heads() {
    let mut fx = SessionFixture::new();
    let (expr, expected) = {
        let mut t = fx.terms();
        let two = t.integer(2);
        let three = t.integer(3);
        let product = t.multiply([two, three]);
        let one = t.integer(1);
        let expr = t.add([one, product]);
        let one2 = t.integer(1);
        let two2 = t.integer(2);
        let three2 = t.integer(3);
        let product2 = t.semantic(SemanticOperator::Multiply, [two2, three2]);
        let expected = t.semantic(SemanticOperator::Add, [one2, product2]);
        (expr, expected)
    };
    assert_structural_eq(fx.session(), expr, expected);
}

#[test]
fn unary_sin_uses_closed_identity() {
    let mut fx = SessionFixture::new();
    let term = {
        let mut t = fx.terms();
        let x = t.symbol("x");
        t.unary_function(UnaryFunction::Sin, x)
    };
    match fx.session().arena.get(term) {
        Some(TermNode::Application {
            head: ApplicationHead::Semantic(op),
            ..
        }) => {
            assert_eq!(op.as_unary(), Some(UnaryFunction::Sin));
        }
        other => panic!("expected unary Sin application, got {other:?}"),
    }
}

#[test]
fn evaluate_add_integers() {
    let mut fx = SessionFixture::new();
    let term = {
        let mut t = fx.terms();
        let a = t.integer(2);
        let b = t.integer(3);
        t.add([a, b])
    };
    let result = fx.evaluate_term(term);
    assert_exact_integer(fx.session(), result, 5);
    let _ = term_request(term);
}
