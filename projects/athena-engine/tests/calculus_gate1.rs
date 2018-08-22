//! Gate 1+ calculus: ConditionalResult, DomainRequest, Abs/Sqrt, limit, series.

use athena_types::{AssumptionSet, DiagnosticCode, Predicate, TermId};

use athena_engine::{
    AthenaEngine, CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder, DomainRequest, LimitApproach,
    LimitDirection, Remainder, Term, differentiate_checked, integrate_checked,
};

#[test]
fn derivative_power_via_domain() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression: Term::app("Power", vec![Term::symbol("x"), Term::int(3)]),
        variable: "x".into(),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact {
            value: CalculusValue::Expression(value),
            ..
        } => {
            let text = format!("{value:?}");
            assert!(text.contains("x") || text.contains("3"), "got {text}");
        }
        other => panic!("expected Exact expression, got {other:?}"),
    }
}

#[test]
fn repeated_derivative() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression: Term::app("Power", vec![Term::symbol("x"), Term::int(3)]),
        variable: "x".into(),
        order: DerivativeOrder::Repeated(2),
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact {
            value: CalculusValue::Expression(value),
            ..
        } => {
            let text = format!("{value:?}");
            assert!(text.contains("x"), "got {text}");
        }
        other => panic!("expected Exact expression, got {other:?}"),
    }
}

#[test]
fn abs_derivative_requires_assumption() {
    let expr = Term::app("Abs", vec![Term::symbol("x")]);
    let unchecked = differentiate_checked(&expr, "x", &AssumptionSet::empty());
    assert!(!unchecked.unresolved.is_empty(), "Abs' must carry unresolved NonZero");

    let with = AssumptionSet::from_predicates(vec![Predicate::NonZero(TermId(0))]);
    let checked = differentiate_checked(&expr, "x", &with);
    assert!(checked.unresolved.is_empty());
}

#[test]
fn integrate_checked_elementary() {
    let expr = Term::app("Power", vec![Term::symbol("x"), Term::int(2)]);
    match integrate_checked(&expr, "x") {
        CalculusResult::Exact { value, .. } => {
            let text = format!("{value:?}");
            assert!(text.contains("x"), "got {text}");
        }
        other => panic!("expected Exact, got {other:?}"),
    }
}

#[test]
fn integrate_checked_unevaluated() {
    let expr = Term::app("Foo", vec![Term::symbol("x")]);
    match integrate_checked(&expr, "x") {
        CalculusResult::Unevaluated { reason, .. } => {
            assert_eq!(reason.code, DiagnosticCode::IntegralNotElementary);
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }
}

#[test]
fn limit_finite_polynomial() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Limit {
        expression: Term::app(
            "Plus",
            vec![
                Term::app("Power", vec![Term::symbol("x"), Term::int(2)]),
                Term::int(1),
            ],
        ),
        variable: "x".into(),
        approach: LimitApproach::Finite(Term::int(2)),
        direction: LimitDirection::TwoSided,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact {
            value: CalculusValue::Expression(value),
            ..
        } => assert_eq!(value, Term::int(5)),
        other => panic!("expected Exact 5, got {other:?}"),
    }
}

#[test]
fn limit_gate1_unevaluated_infinity() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Limit {
        expression: Term::app("Sin", vec![Term::symbol("x")]),
        variable: "x".into(),
        approach: LimitApproach::PositiveInfinity,
        direction: LimitDirection::TwoSided,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Unevaluated { reason, .. } => {
            assert_eq!(reason.code, DiagnosticCode::UnsupportedOperation);
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }
}

#[test]
fn sqrt_derivative_requires_assumption() {
    let expr = Term::app("Sqrt", vec![Term::symbol("x")]);
    let unchecked = differentiate_checked(&expr, "x", &AssumptionSet::empty());
    assert!(!unchecked.unresolved.is_empty());
    let with = AssumptionSet::from_predicates(vec![Predicate::NonNegative(TermId(0))]);
    let checked = differentiate_checked(&expr, "x", &with);
    assert!(checked.unresolved.is_empty());
}

#[test]
fn taylor_polynomial_exact() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Series {
        expression: Term::app(
            "Plus",
            vec![
                Term::app("Power", vec![Term::symbol("x"), Term::int(2)]),
                Term::int(1),
            ],
        ),
        variable: "x".into(),
        center: Term::int(0),
        order: 3,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact {
            value: CalculusValue::Series(series),
            ..
        } => {
            assert_eq!(series.remainder, Remainder::ExactTruncation);
            let t = format!("{:?}", series.to_term());
            assert!(t.contains('1') && t.contains('x'), "got {t}");
        }
        other => panic!("expected Exact Series, got {other:?}"),
    }
}
