//! Bridge `evaluate` coverage (integration tests live next to the crate root).

use athena_engine::{Term, evaluate};

#[test]
fn plus_fold() {
    let e = evaluate(&Term::apply("Plus", vec![Term::int(1), Term::int(2), Term::symbol("x")]));
    assert_eq!(e, Term::apply("Plus", vec![Term::int(3), Term::symbol("x")]));
}

#[test]
fn power_one() {
    let e = evaluate(&Term::apply("Power", vec![Term::symbol("x"), Term::int(1)]));
    assert_eq!(e, Term::symbol("x"));
}

#[test]
fn list_eval() {
    let e = evaluate(&Term::List(vec![Term::int(1), Term::apply("Plus", vec![Term::int(2), Term::int(2)])]));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(4)]));
}

#[test]
fn d_power() {
    let e = evaluate(&Term::apply("D", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(3)]), Term::symbol("x")]));
    assert!(matches!(e, Term::Application { .. }));
    let text = format!("{e:?}");
    assert!(text.contains("x"), "got {text}");
}

#[test]
fn pythagorean() {
    let sin2 = Term::apply("Power", vec![Term::apply("Sin", vec![Term::symbol("x")]), Term::int(2)]);
    let cos2 = Term::apply("Power", vec![Term::apply("Cos", vec![Term::symbol("x")]), Term::int(2)]);
    let e = evaluate(&Term::apply("Simplify", vec![Term::apply("Plus", vec![sin2, cos2])]));
    assert_eq!(e, Term::int(1));
}

#[test]
fn compound_expression_returns_last() {
    let e = evaluate(&Term::apply("CompoundExpression", vec![Term::int(1), Term::int(2), Term::int(3)]));
    assert_eq!(e, Term::int(3));
}

#[test]
fn integrate_power() {
    let e =
        evaluate(&Term::apply("Integrate", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::symbol("x")]));
    let text = format!("{e:?}");
    assert!(text.contains("x"), "got {text}");
}

#[test]
fn map_sin_list() {
    let e = evaluate(&Term::apply("Map", vec![Term::symbol("Sin"), Term::List(vec![Term::int(0)])]));
    assert!(matches!(e, Term::List(_)));
}

#[test]
fn truthy_via_and_or() {
    assert_eq!(evaluate(&Term::apply("And", vec![Term::int(0), Term::int(1)])), Term::int(0));
    assert_eq!(evaluate(&Term::apply("And", vec![Term::int(1), Term::int(1)])), Term::int(1));
    assert_eq!(evaluate(&Term::apply("Or", vec![Term::int(0), Term::int(0)])), Term::int(0));
    assert_eq!(evaluate(&Term::apply("Or", vec![Term::int(0), Term::int(1)])), Term::int(1));
}
