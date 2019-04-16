//! 桥接 `evaluate` 覆盖（集成测试位于 crate 根旁）。

use athena_engine::{Term, evaluate, evaluate_checked};

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
    let e = evaluate(&Term::apply(
        "Integrate",
        vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::symbol("x")],
    ));
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
    assert_eq!(evaluate(&Term::apply("And", vec![Term::int(0), Term::int(1)])), Term::boolean(false));
    assert_eq!(evaluate(&Term::apply("And", vec![Term::int(1), Term::int(1)])), Term::boolean(true));
    assert_eq!(evaluate(&Term::apply("Or", vec![Term::int(0), Term::int(0)])), Term::boolean(false));
    assert_eq!(evaluate(&Term::apply("Or", vec![Term::int(0), Term::int(1)])), Term::boolean(true));
    assert_eq!(
        evaluate(&Term::apply("And", vec![Term::boolean(true), Term::boolean(false)])),
        Term::boolean(false)
    );
    assert_eq!(evaluate(&Term::apply("Not", vec![Term::boolean(true)])), Term::boolean(false));
}

#[test]
fn part_zero_returns_list_head() {
    let e = evaluate(&Term::apply("Part", vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]), Term::int(0)]));
    assert_eq!(e, Term::symbol("List"));
}

#[test]
fn part_oob_is_invalid_index() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::{ComputationStatus, DiagnosticCode};

    let o = evaluate_outcome(&Term::apply("Part", vec![Term::List(vec![Term::int(1), Term::int(2)]), Term::int(9)]));
    assert!(o.has_error());
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::InvalidIndex);
    assert!(evaluate_checked(&Term::apply("Part", vec![Term::List(vec![Term::int(1)]), Term::int(3)])).is_err());
}

#[test]
fn unsupported_import_is_not_silent_value() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::{ComputationStatus, DiagnosticCode};

    let o = evaluate_outcome(&Term::apply("Import", vec![Term::Atom(athena_engine::Atom::String("x.csv".into()))]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::UnsupportedOperation);
}

#[test]
fn unknown_head_is_unevaluated_not_exact_value() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::ComputationStatus;

    let o = evaluate_outcome(&Term::apply("FooBar", vec![Term::int(1)]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert!(!o.has_error());
}

#[test]
fn as_boolean_accepts_true_false_and_bits() {
    use athena_engine::as_boolean;

    assert_eq!(as_boolean(&Term::boolean(true)), Some(true));
    assert_eq!(as_boolean(&Term::boolean(false)), Some(false));
    assert_eq!(as_boolean(&Term::symbol("True")), Some(true));
    assert_eq!(as_boolean(&Term::symbol("False")), Some(false));
    assert_eq!(as_boolean(&Term::int(1)), Some(true));
    assert_eq!(as_boolean(&Term::int(0)), Some(false));
    assert_eq!(as_boolean(&Term::int(2)), None);
    assert_eq!(as_boolean(&Term::symbol("x")), None);
    assert_eq!(as_boolean(&Term::null()), None);
}

#[test]
fn if_true_branch_and_short_circuit() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::DiagnosticCode;

    let e = evaluate(&Term::apply(
        "If",
        vec![Term::apply("Equal", vec![Term::int(1), Term::int(1)]), Term::int(7), Term::int(8)],
    ));
    assert_eq!(e, Term::int(7));

    // False branch must not evaluate Import (would be UnsupportedOperation).
    let o = evaluate_outcome(&Term::apply(
        "If",
        vec![
            Term::symbol("True"),
            Term::int(7),
            Term::apply("Import", vec![Term::Atom(athena_engine::Atom::String("x.csv".into()))]),
        ],
    ));
    assert_eq!(o.term, Term::int(7));
    assert_eq!(o.kind, EvalKind::Value);
    assert!(!o.diagnostics.iter().any(|d| d.code == DiagnosticCode::UnsupportedOperation));
}

#[test]
fn if_false_and_null_and_non_boolean() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::{ComputationStatus, DiagnosticCode};

    assert_eq!(
        evaluate(&Term::apply("If", vec![Term::symbol("False"), Term::int(7), Term::int(8)])),
        Term::int(8)
    );
    assert_eq!(evaluate(&Term::apply("If", vec![Term::int(0), Term::int(7)])), Term::null());

    let o = evaluate_outcome(&Term::apply("If", vec![Term::symbol("x"), Term::int(1), Term::int(2)]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::NonBooleanCondition);
}

#[test]
fn symbol_true_false_null_canonicalize_to_typed_atoms() {
    assert_eq!(evaluate(&Term::symbol("True")), Term::boolean(true));
    assert_eq!(evaluate(&Term::symbol("False")), Term::boolean(false));
    assert_eq!(evaluate(&Term::symbol("Null")), Term::null());
    assert_eq!(
        evaluate(&Term::apply("Equal", vec![Term::int(1), Term::int(1)])),
        Term::boolean(true)
    );
}

#[test]
fn hold_and_hold_form_do_not_eval_args() {
    assert_eq!(
        evaluate(&Term::apply("Hold", vec![Term::apply("Plus", vec![Term::int(1), Term::int(1)])])),
        Term::apply("Hold", vec![Term::apply("Plus", vec![Term::int(1), Term::int(1)])])
    );
    assert_eq!(
        evaluate(&Term::apply("HoldForm", vec![Term::apply("Plus", vec![Term::int(2), Term::int(3)])])),
        Term::apply("HoldForm", vec![Term::apply("Plus", vec![Term::int(2), Term::int(3)])])
    );
}

#[test]
fn which_picks_first_true_branch() {
    let e = evaluate(&Term::apply(
        "Which",
        vec![Term::symbol("False"), Term::int(1), Term::symbol("True"), Term::int(2), Term::symbol("True"), Term::int(3)],
    ));
    assert_eq!(e, Term::int(2));
}

#[test]
fn span_expands_to_list() {
    assert_eq!(
        evaluate(&Term::apply("Span", vec![Term::int(1), Term::int(3)])),
        Term::List(vec![Term::int(1), Term::int(2), Term::int(3)])
    );
    assert_eq!(
        evaluate(&Term::apply("Span", vec![Term::int(1), Term::int(2), Term::int(10)])),
        Term::List(vec![Term::int(1), Term::int(3), Term::int(5), Term::int(7), Term::int(9)])
    );
}

#[test]
fn part_span_slice() {
    let e = evaluate(&Term::apply(
        "Part",
        vec![
            Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]),
            Term::apply("Span", vec![Term::int(1), Term::int(2)]),
        ],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(2)]));
}

#[test]
fn while_false_skips_body() {
    let e = evaluate(&Term::apply("While", vec![Term::int(0), Term::int(1)]));
    assert_eq!(e, Term::null());
}

#[test]
fn compound_set_binds_for_later_stmts() {
    let e = evaluate(&Term::apply(
        "CompoundExpression",
        vec![
            Term::apply("Set", vec![Term::symbol("x"), Term::int(5)]),
            Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)]),
        ],
    ));
    assert_eq!(e, Term::int(6));
}

#[test]
fn part_end_is_last_element() {
    let e = evaluate(&Term::apply(
        "Part",
        vec![Term::List(vec![Term::int(1), Term::int(2), Term::int(3)]), Term::symbol("End")],
    ));
    assert_eq!(e, Term::int(3));
}

#[test]
fn part_all_returns_list() {
    let e = evaluate(&Term::apply(
        "Part",
        vec![Term::List(vec![Term::int(1), Term::int(2)]), Term::symbol("All")],
    ));
    assert_eq!(e, Term::List(vec![Term::int(1), Term::int(2)]));
}

#[test]
fn for_span_last_value() {
    let e = evaluate(&Term::apply(
        "For",
        vec![
            Term::symbol("i"),
            Term::apply("Span", vec![Term::int(1), Term::int(3)]),
            Term::symbol("i"),
        ],
    ));
    assert_eq!(e, Term::int(3));
}

#[test]
fn mldivide_is_unsupported_not_divide() {
    use athena_engine::{EvalKind, evaluate_outcome};
    use athena_types::DiagnosticCode;

    let o = evaluate_outcome(&Term::apply("Mldivide", vec![Term::symbol("A"), Term::symbol("b")]));
    assert_eq!(o.kind, EvalKind::Unevaluated);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::UnsupportedOperation);
    assert!(o.term.head_name() == Some("Mldivide"));
}
