//! 扩展表面名不得变成核心语义分派。

use athena_engine::runtime::values::arena::push_extension;
use athena_ir::{ApplicationHead, Atom, TermNode};
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
    let op = fx.session_mut().extensions.intern("Plus");
    let term = push_extension(fx.session_mut(), op, vec![a, b]);
    match fx.session().arena.get(term) {
        Some(TermNode::Application { head: ApplicationHead::Extension(_), .. }) => {}
        other => panic!("expected Extension head for surface Plus, got {other:?}"),
    }
    let evaluated = fx.evaluate_term(term);
    assert!(
        fx.session().arena.get(evaluated).is_some_and(|n| !matches!(n, TermNode::Atom(Atom::Number(_)))),
        "extension Plus must not evaluate as core Add"
    );
}
