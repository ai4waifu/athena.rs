//! Boundary negatives — dialect surface extension must not become core math.

use athena_engine::runtime::values::arena::push_application_named;
use athena_testing::SessionFixture;

#[test]
fn extension_named_plus_is_not_semantic_add() {
    let mut fx = SessionFixture::new();
    let a = {
        let mut t = fx.terms();
        t.integer(1)
    };
    let b = {
        let mut t = fx.terms();
        t.integer(2)
    };
    // Negative fixture: surface name must stay Extension, never SemanticOperator::Add.
    let term = push_application_named(fx.session_mut(), "Plus", vec![a, b]);
    match fx.session().arena.get(term) {
        Some(athena_ir::TermNode::Application { head: athena_ir::ApplicationHead::Extension(_), .. }) => {}
        other => panic!("expected Extension head for surface Plus, got {other:?}"),
    }
    let evaluated = fx.evaluate_term(term);
    // Must not collapse to integer 3 via string "Plus" dispatch.
    assert!(
        fx.session().arena.get(evaluated).is_some_and(|n| !matches!(n, athena_ir::TermNode::Atom(athena_ir::Atom::Number(_)))),
        "extension Plus must not evaluate as core Add"
    );
}
