//! Gate 1+ calculus: ConditionalResult, DomainRequest, Abs/Sqrt, limit, series.

use athena_types::{AssumptionSet, DiagnosticCode, Predicate, TermId};

use athena_engine::{
    AthenaEngine, CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder, DomainRequest, LimitApproach,
    LimitDirection, Remainder, Term, differentiate_checked, integrate_checked, try_calculus_request,
};

#[test]
fn derivative_power_via_domain() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Derivative {
        expression: Term::apply("Power", vec![Term::symbol("x"), Term::int(3)]),
        variable: "x".into(),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
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
        expression: Term::apply("Power", vec![Term::symbol("x"), Term::int(3)]),
        variable: "x".into(),
        order: DerivativeOrder::Repeated(2),
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            let text = format!("{value:?}");
            assert!(text.contains("x"), "got {text}");
        }
        other => panic!("expected Exact expression, got {other:?}"),
    }
}

#[test]
fn abs_derivative_requires_assumption() {
    let expr = Term::apply("Abs", vec![Term::symbol("x")]);
    let unchecked = differentiate_checked(&expr, "x", &AssumptionSet::empty());
    assert!(!unchecked.unresolved.is_empty(), "Abs' must carry unresolved NonZero");

    let with = AssumptionSet::from_predicates(vec![Predicate::NonZero(TermId(0))]);
    let checked = differentiate_checked(&expr, "x", &with);
    assert!(checked.unresolved.is_empty());
}

#[test]
fn integrate_checked_elementary() {
    let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]);
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
    let expr = Term::apply("Foo", vec![Term::symbol("x")]);
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
        expression: Term::apply("Plus", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::int(1)]),
        variable: "x".into(),
        approach: LimitApproach::Finite(Term::int(2)),
        direction: LimitDirection::TwoSided,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => assert_eq!(value, Term::int(5)),
        other => panic!("expected Exact 5, got {other:?}"),
    }
}

#[test]
fn limit_gate1_unevaluated_infinity() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Limit {
        expression: Term::apply("Sin", vec![Term::symbol("x")]),
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
    let expr = Term::apply("Sqrt", vec![Term::symbol("x")]);
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
        expression: Term::apply("Plus", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::int(1)]),
        variable: "x".into(),
        center: Term::int(0),
        order: 3,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert_eq!(series.remainder, Remainder::ExactTruncation);
            let t = format!("{:?}", series.to_term());
            assert!(t.contains('1') && t.contains('x'), "got {t}");
        }
        other => panic!("expected Exact Series, got {other:?}"),
    }
}

#[test]
fn laurent_simple_pole() {
    let engine = AthenaEngine::new();
    // 1/x about 0 → x^{-1}
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Laurent {
            expression: Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)]),
            variable: "x".into(),
            center: Term::int(0),
            order: 2,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert!(series.terms.iter().any(|(c, p)| *p == -1 && c == &Term::int(1)), "got {:?}", series.terms);
            assert_eq!(series.remainder, Remainder::ExactTruncation);
        }
        other => panic!("expected Laurent Series, got {other:?}"),
    }

    // (1+x)/x about 0 → x^{-1} + 1
    let expr = Term::apply(
        "Times",
        vec![
            Term::apply("Plus", vec![Term::int(1), Term::symbol("x")]),
            Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)]),
        ],
    );
    let term = Term::apply(
        "LaurentSeries",
        vec![expr, Term::List(vec![Term::symbol("x"), Term::int(0), Term::int(2)])],
    );
    let req = try_calculus_request(&term).expect("lower Laurent");
    let out2 = engine.execute_domain(DomainRequest::Calculus(req)).expect("ok");
    match out2 {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            let powers: Vec<i64> = series.terms.iter().map(|(_, p)| *p).collect();
            assert!(powers.contains(&-1), "got {powers:?}");
            assert!(powers.contains(&0), "got {powers:?}");
        }
        other => panic!("expected Laurent Series, got {other:?}"),
    }
}

#[test]
fn residue_simple_poles() {
    let engine = AthenaEngine::new();
    // Res(1/x, 0) = 1
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Residue {
            expression: Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)]),
            variable: "x".into(),
            point: Term::int(0),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Residue(r), .. } => {
            assert_eq!(r.value, Term::int(1));
            assert_eq!(r.pole_order, 1);
        }
        other => panic!("expected Residue Exact, got {other:?}"),
    }

    // Res(1/x^2, 0) = 0（二阶极点，无 x^{-1}）
    let out2 = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Residue {
            expression: Term::apply("Power", vec![Term::symbol("x"), Term::int(-2)]),
            variable: "x".into(),
            point: Term::int(0),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out2 {
        CalculusResult::Exact { value: CalculusValue::Residue(r), .. } => {
            assert_eq!(r.value, Term::int(0));
            assert_eq!(r.pole_order, 2);
        }
        other => panic!("expected Residue Exact for 1/x^2, got {other:?}"),
    }

    // Residue[(1+x)/x, {x, 0}] → 1
    let expr = Term::apply(
        "Times",
        vec![
            Term::apply("Plus", vec![Term::int(1), Term::symbol("x")]),
            Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)]),
        ],
    );
    let term = Term::apply("Residue", vec![expr, Term::List(vec![Term::symbol("x"), Term::int(0)])]);
    let req = try_calculus_request(&term).expect("lower Residue");
    let out3 = engine.execute_domain(DomainRequest::Calculus(req)).expect("ok");
    match out3 {
        CalculusResult::Exact { value: CalculusValue::Residue(r), .. } => {
            assert_eq!(r.value, Term::int(1));
        }
        other => panic!("expected Residue from lower, got {other:?}"),
    }
}

#[test]
fn special_function_registry_derivatives() {
    // Sinh' = Cosh
    let d_sinh = athena_engine::differentiate(&Term::apply("Sinh", vec![Term::symbol("x")]), "x");
    assert_eq!(d_sinh, Term::apply("Cosh", vec![Term::symbol("x")]));

    // ArcTan' = 1/(1+x^2)
    let d_atan = athena_engine::differentiate(&Term::apply("ArcTan", vec![Term::symbol("x")]), "x");
    let text = format!("{d_atan:?}");
    assert!(text.contains('1') || text.contains("Power"), "got {text}");

    // Erf' contains Exp and Pi
    let d_erf = athena_engine::differentiate(&Term::apply("Erf", vec![Term::symbol("x")]), "x");
    let text = format!("{d_erf:?}");
    assert!(text.contains("Exp") && text.contains("Pi"), "got {text}");

    // Gamma' = Gamma * PolyGamma[0, ·]
    let d_gamma = athena_engine::differentiate(&Term::apply("Gamma", vec![Term::symbol("x")]), "x");
    let text = format!("{d_gamma:?}");
    assert!(text.contains("Gamma") && text.contains("PolyGamma"), "got {text}");

    assert!(athena_engine::lookup_function("Erf").is_some());
    assert!(athena_engine::registered_function_names().any(|n| n == "Gamma"));
}

#[test]
fn asymptotic_at_infinity() {
    let engine = AthenaEngine::new();
    // x^2 + 1 as x→∞
    let poly = Term::apply("Plus", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::int(1)]);
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Asymptotic {
            expression: poly,
            variable: "x".into(),
            order: 2,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert!(series.center.is_symbol("Infinity"));
            let powers: Vec<i64> = series.terms.iter().map(|(_, p)| *p).collect();
            assert!(powers.contains(&2), "got {powers:?}");
            assert!(powers.contains(&0), "got {powers:?}");
            assert_eq!(series.remainder, Remainder::ExactTruncation);
        }
        other => panic!("expected asymptotic Series, got {other:?}"),
    }

    // 1/(x+1) ~ x^{-1} - x^{-2} + …
    let rat = Term::apply(
        "Power",
        vec![Term::apply("Plus", vec![Term::symbol("x"), Term::int(1)]), Term::int(-1)],
    );
    let term = Term::apply(
        "Asymptotic",
        vec![rat, Term::List(vec![Term::symbol("x"), Term::symbol("Infinity"), Term::int(2)])],
    );
    let req = try_calculus_request(&term).expect("lower Asymptotic");
    let out2 = engine.execute_domain(DomainRequest::Calculus(req)).expect("ok");
    match out2 {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert!(series.center.is_symbol("Infinity"));
            let powers: Vec<i64> = series.terms.iter().map(|(_, p)| *p).collect();
            assert!(powers.contains(&-1), "got {powers:?}");
            assert!(powers.iter().any(|p| *p <= -1), "got {powers:?}");
        }
        other => panic!("expected rational asymptotic, got {other:?}"),
    }
}

#[test]
fn limit_poly_at_infinity() {
    let engine = AthenaEngine::new();
    let req = DomainRequest::Calculus(CalculusRequest::Limit {
        expression: Term::apply("Plus", vec![Term::apply("Times", vec![Term::int(-2), Term::symbol("x")]), Term::int(5)]),
        variable: "x".into(),
        approach: LimitApproach::PositiveInfinity,
        direction: LimitDirection::TwoSided,
        assumptions: AssumptionSet::empty(),
    });
    let out = engine.execute_domain(req).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(value, Term::apply("Times", vec![Term::int(-1), Term::symbol("Infinity")]));
        }
        other => panic!("expected -Infinity, got {other:?}"),
    }
}

#[test]
fn onesided_simple_pole() {
    let engine = AthenaEngine::new();
    let expr = Term::apply("Divide", vec![Term::int(1), Term::symbol("x")]);
    let above = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Limit {
            expression: expr.clone(),
            variable: "x".into(),
            approach: LimitApproach::Finite(Term::int(0)),
            direction: LimitDirection::FromAbove,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match above {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => assert_eq!(value, Term::symbol("Infinity")),
        other => panic!("expected +Infinity, got {other:?}"),
    }
    let below = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::Finite(Term::int(0)),
            direction: LimitDirection::FromBelow,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match below {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(value, Term::apply("Times", vec![Term::int(-1), Term::symbol("Infinity")]))
        }
        other => panic!("expected -Infinity, got {other:?}"),
    }
}

#[test]
fn definite_integral_power() {
    let engine = AthenaEngine::new();
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::DefiniteIntegral {
            expression: Term::symbol("x"),
            variable: "x".into(),
            lower: Term::int(0),
            upper: Term::int(2),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => assert_eq!(value, Term::int(2)),
        other => panic!("expected Exact 2, got {other:?}"),
    }
}

#[test]
fn taylor_nonzero_center() {
    let engine = AthenaEngine::new();
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Series {
            expression: Term::apply(
                "Power",
                vec![
                    Term::apply("Plus", vec![Term::symbol("x"), Term::apply("Times", vec![Term::int(-1), Term::int(1)])]),
                    Term::int(2),
                ],
            ),
            variable: "x".into(),
            center: Term::int(1),
            order: 3,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert_eq!(series.remainder, Remainder::ExactTruncation);
            assert_eq!(series.center, Term::int(1));
        }
        other => panic!("expected Exact Series, got {other:?}"),
    }
}

#[test]
fn gradient_of_quadratic() {
    let engine = AthenaEngine::new();
    let expr = Term::apply(
        "Plus",
        vec![
            Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]),
            Term::apply("Power", vec![Term::symbol("y"), Term::int(2)]),
        ],
    );
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Gradient {
            expression: expr,
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Gradient(g), .. } => {
            assert_eq!(g.variables, vec!["x".to_string(), "y".to_string()]);
            assert_eq!(g.components.len(), 2);
            let cx = format!("{:?}", g.components[0]);
            let cy = format!("{:?}", g.components[1]);
            assert!(cx.contains('x'), "got {cx}");
            assert!(cy.contains('y'), "got {cy}");
        }
        other => panic!("expected Gradient, got {other:?}"),
    }
}

#[test]
fn jacobian_linear_map() {
    let engine = AthenaEngine::new();
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Jacobian {
            expressions: vec![Term::apply("Plus", vec![Term::symbol("x"), Term::symbol("y")]), Term::symbol("x")],
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Jacobian(j), .. } => {
            assert_eq!(j.rows.len(), 2);
            assert_eq!(j.rows[0], vec![Term::int(1), Term::int(1)]);
            assert_eq!(j.rows[1], vec![Term::int(1), Term::int(0)]);
        }
        other => panic!("expected Jacobian, got {other:?}"),
    }
}

#[test]
fn hessian_quadratic() {
    let engine = AthenaEngine::new();
    let expr = Term::apply(
        "Plus",
        vec![
            Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]),
            Term::apply("Times", vec![Term::symbol("x"), Term::symbol("y")]),
        ],
    );
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Hessian {
            expression: expr,
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Hessian(h), .. } => {
            assert_eq!(h.entries.len(), 2);
            assert_eq!(h.entries[0][0], Term::int(2));
            assert_eq!(h.entries[0][1], Term::int(1));
            assert_eq!(h.entries[1][0], Term::int(1));
            assert_eq!(h.entries[1][1], Term::int(0));
        }
        other => panic!("expected Hessian, got {other:?}"),
    }
}

#[test]
fn divergence_of_linear_field() {
    let engine = AthenaEngine::new();
    // F = (x, y) ⇒ div = 2
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Divergence {
            components: vec![Term::symbol("x"), Term::symbol("y")],
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Divergence(d), .. } => {
            assert_eq!(d.value, Term::int(2));
        }
        other => panic!("expected Divergence, got {other:?}"),
    }
}

#[test]
fn curl_of_linear_3d_field() {
    let engine = AthenaEngine::new();
    // F = (−y, x, 0) ⇒ curl = (0, 0, 2)
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Curl {
            components: vec![Term::apply("Times", vec![Term::int(-1), Term::symbol("y")]), Term::symbol("x"), Term::int(0)],
            variables: vec!["x".into(), "y".into(), "z".into()],
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Curl(c), .. } => {
            assert_eq!(c.curl_components, vec![Term::int(0), Term::int(0), Term::int(2)]);
        }
        other => panic!("expected Curl, got {other:?}"),
    }
}

#[test]
fn divergence_via_term_lowering() {
    let engine = AthenaEngine::new();
    let term = Term::apply(
        "Divergence",
        vec![
            Term::List(vec![Term::symbol("x"), Term::symbol("y"), Term::symbol("z")]),
            Term::List(vec![Term::symbol("x"), Term::symbol("y"), Term::symbol("z")]),
        ],
    );
    let req = try_calculus_request(&term).expect("lower");
    let out = engine.execute_domain(DomainRequest::Calculus(req)).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Divergence(d), .. } => {
            assert_eq!(d.value, Term::int(3));
        }
        other => panic!("expected Divergence, got {other:?}"),
    }
}

#[test]
fn ode_y_prime_equals_const() {
    let engine = AthenaEngine::new();
    let eq = Term::apply("Equal", vec![Term::apply("D", vec![Term::symbol("y"), Term::symbol("x")]), Term::int(2)]);
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, athena_engine::VerificationStatus::Verified { .. }));
            assert_eq!(sol.explicit, Term::apply("Times", vec![Term::int(2), Term::symbol("x")]));
        }
        other => panic!("expected verified ODE solution, got {other:?}"),
    }
}

#[test]
fn ode_ivp_y_prime_const() {
    let engine = AthenaEngine::new();
    let eq = Term::apply("Equal", vec![Term::apply("D", vec![Term::symbol("y"), Term::symbol("x")]), Term::int(2)]);
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: Some((Term::int(0), Term::int(1))),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, athena_engine::VerificationStatus::Verified { .. }));
            // y = 2x + 1
            let text = format!("{:?}", sol.explicit);
            assert!(text.contains('x') && text.contains('1'), "got {text}");
        }
        other => panic!("expected IVP solution, got {other:?}"),
    }
}

#[test]
fn ode_separable_g_of_x() {
    let engine = AthenaEngine::new();
    // y' = x ⇒ y = x^2/2
    let eq = Term::apply("Equal", vec![Term::apply("D", vec![Term::symbol("y"), Term::symbol("x")]), Term::symbol("x")]);
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, athena_engine::VerificationStatus::Verified { .. }));
            let text = format!("{:?}", sol.explicit);
            assert!(text.contains('x'), "got {text}");
        }
        other => panic!("expected separable g(x) solution, got {other:?}"),
    }
}

#[test]
fn ode_power_y_squared() {
    let engine = AthenaEngine::new();
    // y' = y^2 ⇒ y = -1/x
    let eq = Term::apply(
        "Equal",
        vec![
            Term::apply("D", vec![Term::symbol("y"), Term::symbol("x")]),
            Term::apply("Power", vec![Term::symbol("y"), Term::int(2)]),
        ],
    );
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, athena_engine::VerificationStatus::Verified { .. }));
            assert_eq!(
                sol.explicit,
                Term::apply("Times", vec![Term::int(-1), Term::apply("Power", vec![Term::symbol("x"), Term::int(-1)])])
            );
        }
        other => panic!("expected y=-1/x, got {other:?}"),
    }
}

#[test]
fn ode_bernoulli_const_and_separable_xy2() {
    let engine = AthenaEngine::new();
    // y' = 2y + y^2 ⇒ y = -2
    let eq = Term::apply(
        "Equal",
        vec![
            Term::apply("D", vec![Term::symbol("y"), Term::symbol("x")]),
            Term::apply(
                "Plus",
                vec![
                    Term::apply("Times", vec![Term::int(2), Term::symbol("y")]),
                    Term::apply("Power", vec![Term::symbol("y"), Term::int(2)]),
                ],
            ),
        ],
    );
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, athena_engine::VerificationStatus::Verified { .. }));
            assert_eq!(sol.explicit, Term::int(-2));
        }
        other => panic!("expected Bernoulli constant y=-2, got {other:?}"),
    }

    // y' = x y^2 ⇒ y = -1/(x^2/2) = -2/x^2
    let eq2 = Term::apply(
        "Equal",
        vec![
            Term::apply("D", vec![Term::symbol("y"), Term::symbol("x")]),
            Term::apply("Times", vec![Term::symbol("x"), Term::apply("Power", vec![Term::symbol("y"), Term::int(2)])]),
        ],
    );
    let out2 = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::SolveOde {
            equation: eq2,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out2 {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, athena_engine::VerificationStatus::Verified { .. }));
            let text = format!("{:?}", sol.explicit);
            assert!(text.contains('x'), "got {text}");
        }
        other => panic!("expected separable x y^2 solution, got {other:?}"),
    }
}

#[test]
fn laplace_exp_and_sin() {
    let engine = AthenaEngine::new();
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Laplace,
            expression: Term::apply("Exp", vec![Term::apply("Times", vec![Term::int(2), Term::symbol("t")])]),
            time_variable: "t".into(),
            transform_variable: "s".into(),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            let text = format!("{:?}", tr.expression);
            assert!(text.contains('s'), "got {text}");
        }
        other => panic!("expected Laplace Transform, got {other:?}"),
    }

    let sin = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Laplace,
            expression: Term::apply("Sin", vec![Term::symbol("t")]),
            time_variable: "t".into(),
            transform_variable: "s".into(),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    assert!(matches!(sin, CalculusResult::Exact { value: CalculusValue::Transform(_), .. }));
}

#[test]
fn fourier_exp_abs_decay() {
    let engine = AthenaEngine::new();
    // Exp[-Abs[t]] → 2/(1+ω²)
    let expr = Term::apply("Exp", vec![Term::apply("Times", vec![Term::int(-1), Term::apply("Abs", vec![Term::symbol("t")])])]);
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Fourier,
            expression: expr,
            time_variable: "t".into(),
            transform_variable: "w".into(),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            assert_eq!(tr.kind, athena_engine::TransformKind::Fourier);
            let text = format!("{:?}", tr.expression);
            assert!(text.contains('w'), "got {text}");
            // 2 / (1 + w^2)
            assert_eq!(
                tr.expression,
                Term::apply(
                    "Times",
                    vec![
                        Term::int(2),
                        Term::apply(
                            "Power",
                            vec![
                                Term::apply(
                                    "Plus",
                                    vec![Term::int(1), Term::apply("Power", vec![Term::symbol("w"), Term::int(2)])],
                                ),
                                Term::int(-1),
                            ],
                        ),
                    ],
                )
            );
        }
        other => panic!("expected Fourier Transform, got {other:?}"),
    }
}

#[test]
fn fourier_gaussian_and_lowering() {
    let engine = AthenaEngine::new();
    let expr = Term::apply(
        "Exp",
        vec![Term::apply("Times", vec![Term::int(-1), Term::apply("Power", vec![Term::symbol("t"), Term::int(2)])])],
    );
    let term = Term::apply("FourierTransform", vec![expr.clone(), Term::symbol("t"), Term::symbol("w")]);
    let req = try_calculus_request(&term).expect("lower Fourier");
    let out = engine.execute_domain(DomainRequest::Calculus(req)).expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            let text = format!("{:?}", tr.expression);
            assert!(text.contains("Sqrt") || text.contains("Pi") || text.contains('w'), "got {text}");
        }
        other => panic!("expected Fourier Transform, got {other:?}"),
    }

    let causal = Term::apply(
        "Times",
        vec![
            Term::apply("UnitStep", vec![Term::symbol("t")]),
            Term::apply("Exp", vec![Term::apply("Times", vec![Term::int(-2), Term::symbol("t")])]),
        ],
    );
    let causal_out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Fourier,
            expression: causal,
            time_variable: "t".into(),
            transform_variable: "w".into(),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match causal_out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            let text = format!("{:?}", tr.expression);
            assert!(text.contains('I') && text.contains('w'), "got {text}");
        }
        other => panic!("expected causal Fourier, got {other:?}"),
    }
}

#[test]
fn z_transform_geometric_and_delta() {
    let engine = AthenaEngine::new();
    // 2^n → z/(z-2), |z|>2
    let geom = Term::apply("Power", vec![Term::int(2), Term::symbol("n")]);
    let out = engine
        .execute_domain(DomainRequest::Calculus(CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Z,
            expression: geom,
            time_variable: "n".into(),
            transform_variable: "z".into(),
            assumptions: AssumptionSet::empty(),
        }))
        .expect("ok");
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            assert_eq!(tr.kind, athena_engine::TransformKind::Z);
            let text = format!("{:?}", tr.expression);
            assert!(text.contains('z'), "got {text}");
            let roc = format!("{:?}", tr.region_of_convergence.predicate);
            assert!(roc.contains("Abs"), "got {roc}");
        }
        other => panic!("expected Z Transform, got {other:?}"),
    }

    let delta = Term::apply("KroneckerDelta", vec![Term::symbol("n")]);
    let term = Term::apply("ZTransform", vec![delta, Term::symbol("n"), Term::symbol("z")]);
    let req = try_calculus_request(&term).expect("lower Z");
    let delta_out = engine.execute_domain(DomainRequest::Calculus(req)).expect("ok");
    match delta_out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert_eq!(tr.expression, Term::int(1));
            assert!(tr.region_of_convergence.known);
        }
        other => panic!("expected delta Z, got {other:?}"),
    }
}

#[test]
fn try_calculus_request_d_limit_series() {
    let d = Term::apply("D", vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]), Term::symbol("x")]);
    assert!(matches!(athena_engine::try_calculus_request(&d), Some(CalculusRequest::Derivative { .. })));
    let lim = Term::apply("Limit", vec![Term::symbol("x"), Term::apply("Rule", vec![Term::symbol("x"), Term::int(1)])]);
    assert!(matches!(athena_engine::try_calculus_request(&lim), Some(CalculusRequest::Limit { .. })));
    let series = Term::apply(
        "Series",
        vec![
            Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]),
            Term::List(vec![Term::symbol("x"), Term::int(0), Term::int(3)]),
        ],
    );
    assert!(matches!(athena_engine::try_calculus_request(&series), Some(CalculusRequest::Series { .. })));
}

#[test]
fn evaluate_routes_d_through_domain() {
    let e = athena_engine::evaluate(&Term::apply(
        "D",
        vec![Term::apply("Power", vec![Term::symbol("x"), Term::int(3)]), Term::symbol("x")],
    ));
    let text = format!("{e:?}");
    assert!(text.contains('x'), "got {text}");
}
