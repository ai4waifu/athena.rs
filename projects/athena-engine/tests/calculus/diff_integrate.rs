//! 微积分合同：`ConditionalResult`、`DomainRequest`、Abs/Sqrt、极限、级数（arena `TermId` · Living `25`）。

use std::cell::RefCell;

use athena_types::{AssumptionSet, DiagnosticCode, Predicate, TermId};

use athena_engine::{
    AthenaEngine, CalculusCtx, CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder, DomainRequest, DomainResult,
    LimitApproach, LimitDirection, Remainder, Session, VerificationStatus,
    arena_ops::{push_app_named, push_int, push_list, push_symbol_name},
    differentiate, differentiate_checked, integrate_checked, present::term_debug, try_calculus_request,
};

type Tid = TermId;

struct H {
    s: RefCell<Session>,
}

impl H {
    fn new() -> Self {
        Self { s: RefCell::new(Session::new()) }
    }

    fn sym(&self, name: &str) -> Tid {
        push_symbol_name(&mut self.s.borrow_mut(), name)
    }

    fn i(&self, n: i64) -> Tid {
        push_int(&mut self.s.borrow_mut(), n)
    }

    fn ap(&self, head: &str, args: Vec<Tid>) -> Tid {
        push_app_named(&mut self.s.borrow_mut(), head, args)
    }

    fn lst(&self, items: Vec<Tid>) -> Tid {
        push_list(&mut self.s.borrow_mut(), items)
    }

    fn dbg(&self, id: Tid) -> String {
        term_debug(&self.s.borrow(), id)
    }

    fn eq(&self, a: Tid, b: Tid) -> bool {
        self.s.borrow().arena.structural_eq(a, b)
    }

    fn with_session<R>(&self, f: impl FnOnce(&mut Session) -> R) -> R {
        f(&mut self.s.borrow_mut())
    }

    fn with_cc<R>(&self, f: impl FnOnce(&mut CalculusCtx<'_>) -> R) -> R {
        self.with_session(|s| {
            let mut cc = CalculusCtx::new(s);
            f(&mut cc)
        })
    }
}

fn expect_calculus(r: Result<DomainResult, athena_types::Diagnostic>) -> CalculusResult<CalculusValue> {
    match r.expect("domain ok") {
        DomainResult::Calculus(c) => c,
        other => panic!("expected Calculus domain, got {other:?}"),
    }
}

fn run(engine: &AthenaEngine, h: &H, req: CalculusRequest) -> CalculusResult<CalculusValue> {
    h.with_session(|s| expect_calculus(engine.execute_domain(s, DomainRequest::Calculus(req))))
}

fn lower(h: &H, root: Tid) -> CalculusRequest {
    h.with_cc(|cc| try_calculus_request(cc, root).expect("lower calculus request"))
}

#[test]
fn derivative_power_via_domain() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap("Power", vec![h.sym("x"), h.i(3)]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Derivative {
            expression: expr,
            variable: "x".into(),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            let text = h.dbg(value);
            assert!(text.contains('x') || text.contains('3'), "got {text}");
        }
        other => panic!("expected Exact expression, got {other:?}"),
    }
}

#[test]
fn repeated_derivative() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap("Power", vec![h.sym("x"), h.i(3)]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Derivative {
            expression: expr,
            variable: "x".into(),
            order: DerivativeOrder::Repeated(2),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            let text = h.dbg(value);
            assert!(text.contains('x'), "got {text}");
        }
        other => panic!("expected Exact expression, got {other:?}"),
    }
}

#[test]
fn abs_derivative_requires_assumption() {
    let h = H::new();
    let expr = h.ap("Abs", vec![h.sym("x")]);
    let unchecked = h.with_cc(|cc| differentiate_checked(cc, expr, "x", &AssumptionSet::empty()));
    assert!(!unchecked.unresolved.is_empty(), "Abs' must carry unresolved NonZero");
    let with = AssumptionSet::from_predicates(vec![Predicate::NonZero(TermId(0))]);
    let checked = h.with_cc(|cc| differentiate_checked(cc, expr, "x", &with));
    assert!(checked.unresolved.is_empty());
}

#[test]
fn integrate_checked_elementary() {
    let h = H::new();
    let expr = h.ap("Power", vec![h.sym("x"), h.i(2)]);
    match h.with_cc(|cc| integrate_checked(cc, expr, "x")) {
        CalculusResult::Exact { value, .. } => {
            let text = h.dbg(value);
            assert!(text.contains('x'), "got {text}");
        }
        other => panic!("expected Exact, got {other:?}"),
    }
}

#[test]
fn integrate_checked_unevaluated() {
    let h = H::new();
    let expr = h.ap("Foo", vec![h.sym("x")]);
    match h.with_cc(|cc| integrate_checked(cc, expr, "x")) {
        CalculusResult::Unevaluated { reason, .. } => {
            assert_eq!(reason.code, DiagnosticCode::IntegralNotElementary);
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }
}

#[test]
fn limit_finite_polynomial() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap("Plus", vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.i(1)]);
    let approach = h.i(2);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::Finite(approach),
            direction: LimitDirection::TwoSided,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "5");
        }
        other => panic!("expected Exact 5, got {other:?}"),
    }
}

#[test]
fn limit_unevaluated_infinity() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap("Sin", vec![h.sym("x")]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::PositiveInfinity,
            direction: LimitDirection::TwoSided,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Unevaluated { reason, .. } => {
            assert_eq!(reason.code, DiagnosticCode::UnsupportedOperation);
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }
}

#[test]
fn sqrt_derivative_requires_assumption() {
    let h = H::new();
    let expr = h.ap("Sqrt", vec![h.sym("x")]);
    let unchecked = h.with_cc(|cc| differentiate_checked(cc, expr, "x", &AssumptionSet::empty()));
    assert!(!unchecked.unresolved.is_empty());
    let with = AssumptionSet::from_predicates(vec![Predicate::NonNegative(TermId(0))]);
    let checked = h.with_cc(|cc| differentiate_checked(cc, expr, "x", &with));
    assert!(checked.unresolved.is_empty());
}

#[test]
fn taylor_polynomial_exact() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap("Plus", vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.i(1)]);
    let center = h.i(0);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Series {
            expression: expr,
            variable: "x".into(),
            center,
            order: 3,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert_eq!(series.remainder, Remainder::ExactTruncation);
            let tid = h.with_cc(|cc| series.to_term(cc));
            let t = h.dbg(tid);
            assert!(t.contains('1') && t.contains('x'), "got {t}");
        }
        other => panic!("expected Exact Series, got {other:?}"),
    }
}

#[test]
fn laurent_simple_pole() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // 1/x 在 0 展开 → x^{-1}
    let out = run(
        &engine,
        &h,
        CalculusRequest::Laurent {
            expression: h.ap("Power", vec![h.sym("x"), h.i(-1)]),
            variable: "x".into(),
            center: h.i(0),
            order: 2,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert!(
                series.terms.iter().any(|(c, p)| *p == -1 && h.dbg(*c) == "1"),
                "got {:?}",
                series.terms.iter().map(|(c, p)| (h.dbg(*c), *p)).collect::<Vec<_>>()
            );
            assert_eq!(series.remainder, Remainder::ExactTruncation);
        }
        other => panic!("expected Laurent Series, got {other:?}"),
    }

    // (1+x)/x 在 0 展开 → x^{-1} + 1
    let expr = h.ap(
        "Times",
        vec![h.ap("Plus", vec![h.i(1), h.sym("x")]), h.ap("Power", vec![h.sym("x"), h.i(-1)])],
    );
    let term = h.ap("LaurentSeries", vec![expr, h.lst(vec![h.sym("x"), h.i(0), h.i(2)])]);
    let req = lower(&h, term);
    let out2 = run(&engine, &h, req);
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
    let h = H::new();
    // 留数：Res(1/x, 0) = 1
    let out = run(
        &engine,
        &h,
        CalculusRequest::Residue {
            expression: h.ap("Power", vec![h.sym("x"), h.i(-1)]),
            variable: "x".into(),
            point: h.i(0),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Residue(r), .. } => {
            assert_eq!(h.dbg(r.value), "1");
            assert_eq!(r.pole_order, 1);
        }
        other => panic!("expected Residue Exact, got {other:?}"),
    }

    // 留数：Res(1/x², 0) = 0（二阶极点，无 x⁻¹）
    let out2 = run(
        &engine,
        &h,
        CalculusRequest::Residue {
            expression: h.ap("Power", vec![h.sym("x"), h.i(-2)]),
            variable: "x".into(),
            point: h.i(0),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out2 {
        CalculusResult::Exact { value: CalculusValue::Residue(r), .. } => {
            assert_eq!(h.dbg(r.value), "0");
            assert_eq!(r.pole_order, 2);
        }
        other => panic!("expected Residue Exact for 1/x^2, got {other:?}"),
    }

    // 形态：Residue[(1+x)/x, {x, 0}] → 1
    let expr = h.ap(
        "Times",
        vec![h.ap("Plus", vec![h.i(1), h.sym("x")]), h.ap("Power", vec![h.sym("x"), h.i(-1)])],
    );
    let term = h.ap("Residue", vec![expr, h.lst(vec![h.sym("x"), h.i(0)])]);
    let req = lower(&h, term);
    let out3 = run(&engine, &h, req);
    match out3 {
        CalculusResult::Exact { value: CalculusValue::Residue(r), .. } => {
            assert_eq!(h.dbg(r.value), "1");
        }
        other => panic!("expected Residue from lower, got {other:?}"),
    }
}

#[test]
fn special_function_registry_derivatives() {
    let h = H::new();
    // 导数：Sinh' = Cosh
    let sinh = h.ap("Sinh", vec![h.sym("x")]);
    let d_sinh = h.with_cc(|cc| differentiate(cc, sinh, "x"));
    let cosh = h.ap("Cosh", vec![h.sym("x")]);
    assert!(h.eq(d_sinh, cosh), "got {}", h.dbg(d_sinh));

    // 导数：ArcTan' = 1/(1+x^2)
    let atan = h.ap("ArcTan", vec![h.sym("x")]);
    let d_atan = h.with_cc(|cc| differentiate(cc, atan, "x"));
    let text = h.dbg(d_atan);
    assert!(text.contains('1') || text.contains("Power"), "got {text}");

    // 误差函数：Erf' 含 Exp 与 Pi
    let erf = h.ap("Erf", vec![h.sym("x")]);
    let d_erf = h.with_cc(|cc| differentiate(cc, erf, "x"));
    let text = h.dbg(d_erf);
    assert!(text.contains("Exp") && text.contains("Pi"), "got {text}");

    // 导数：Gamma' = Gamma * PolyGamma[0, ·]
    let gamma = h.ap("Gamma", vec![h.sym("x")]);
    let d_gamma = h.with_cc(|cc| differentiate(cc, gamma, "x"));
    let text = h.dbg(d_gamma);
    assert!(text.contains("Gamma") && text.contains("PolyGamma"), "got {text}");

    assert!(athena_engine::lookup_function("Erf").is_some());
    assert!(athena_engine::registered_function_names().any(|n| n == "Gamma"));
}

#[test]
fn asymptotic_at_infinity() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // 极限：x²+1，x→∞
    let poly = h.ap("Plus", vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.i(1)]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Asymptotic {
            expression: poly,
            variable: "x".into(),
            order: 2,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert_eq!(h.dbg(series.center), "Infinity");
            let powers: Vec<i64> = series.terms.iter().map(|(_, p)| *p).collect();
            assert!(powers.contains(&2), "got {powers:?}");
            assert!(powers.contains(&0), "got {powers:?}");
            assert_eq!(series.remainder, Remainder::ExactTruncation);
        }
        other => panic!("expected asymptotic Series, got {other:?}"),
    }

    // 1/(x+1) ~ x^{-1} - x^{-2} + …
    let rat = h.ap("Power", vec![h.ap("Plus", vec![h.sym("x"), h.i(1)]), h.i(-1)]);
    let term = h.ap("Asymptotic", vec![rat, h.lst(vec![h.sym("x"), h.sym("Infinity"), h.i(2)])]);
    let req = lower(&h, term);
    let out2 = run(&engine, &h, req);
    match out2 {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert_eq!(h.dbg(series.center), "Infinity");
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
    let h = H::new();
    let expr = h.ap("Plus", vec![h.ap("Times", vec![h.i(-2), h.sym("x")]), h.i(5)]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::PositiveInfinity,
            direction: LimitDirection::TwoSided,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "Times[-1, Infinity]");
        }
        other => panic!("expected -Infinity, got {other:?}"),
    }
}

#[test]
fn onesided_simple_pole() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap("Divide", vec![h.i(1), h.sym("x")]);
    let above = run(
        &engine,
        &h,
        CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::Finite(h.i(0)),
            direction: LimitDirection::FromAbove,
            assumptions: AssumptionSet::empty(),
        },
    );
    match above {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "Infinity");
        }
        other => panic!("expected +Infinity, got {other:?}"),
    }
    let below = run(
        &engine,
        &h,
        CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::Finite(h.i(0)),
            direction: LimitDirection::FromBelow,
            assumptions: AssumptionSet::empty(),
        },
    );
    match below {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "Times[-1, Infinity]");
        }
        other => panic!("expected -Infinity, got {other:?}"),
    }
}

#[test]
fn definite_integral_power() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let out = run(
        &engine,
        &h,
        CalculusRequest::DefiniteIntegral {
            expression: h.sym("x"),
            variable: "x".into(),
            lower: h.i(0),
            upper: h.i(2),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "2");
        }
        other => panic!("expected Exact 2, got {other:?}"),
    }
}

#[test]
fn limit_sinc_at_zero() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap(
        "Times",
        vec![h.ap("Sin", vec![h.sym("x")]), h.ap("Power", vec![h.sym("x"), h.i(-1)])],
    );
    let out = run(
        &engine,
        &h,
        CalculusRequest::Limit {
            expression: expr,
            variable: "x".into(),
            approach: LimitApproach::Finite(h.i(0)),
            direction: LimitDirection::TwoSided,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "1");
        }
        other => panic!("expected Exact 1, got {other:?}"),
    }
}

#[test]
fn definite_integral_sin_zero_to_pi() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let out = run(
        &engine,
        &h,
        CalculusRequest::DefiniteIntegral {
            expression: h.ap("Sin", vec![h.sym("x")]),
            variable: "x".into(),
            lower: h.i(0),
            upper: h.sym("Pi"),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Expression(value), .. } => {
            assert_eq!(h.dbg(value), "2");
        }
        other => panic!("expected Exact 2, got {other:?}"),
    }
}

#[test]
fn cos_pi_is_exact_minus_one() {
    let h = H::new();
    let pi = h.sym("Pi");
    let cos = h.ap("Cos", vec![pi]);
    let cos_out = h.with_session(|s| athena_engine::interp::vm::evaluate_session(s, cos));
    assert_eq!(h.dbg(cos_out.term), "-1");
    let sin = h.ap("Sin", vec![pi]);
    let sin_out = h.with_session(|s| athena_engine::interp::vm::evaluate_session(s, sin));
    assert_eq!(h.dbg(sin_out.term), "0");
}

#[test]
fn taylor_nonzero_center() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap(
        "Power",
        vec![h.ap("Plus", vec![h.sym("x"), h.ap("Times", vec![h.i(-1), h.i(1)])]), h.i(2)],
    );
    let center = h.i(1);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Series {
            expression: expr,
            variable: "x".into(),
            center,
            order: 3,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Series(series), .. } => {
            assert_eq!(series.remainder, Remainder::ExactTruncation);
            assert_eq!(h.dbg(series.center), "1");
        }
        other => panic!("expected Exact Series, got {other:?}"),
    }
}

#[test]
fn gradient_of_quadratic() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap(
        "Plus",
        vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.ap("Power", vec![h.sym("y"), h.i(2)])],
    );
    let out = run(
        &engine,
        &h,
        CalculusRequest::Gradient {
            expression: expr,
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Gradient(g), .. } => {
            assert_eq!(g.variables, vec!["x".to_string(), "y".to_string()]);
            assert_eq!(g.components.len(), 2);
            let cx = h.dbg(g.components[0]);
            let cy = h.dbg(g.components[1]);
            assert!(cx.contains('x'), "got {cx}");
            assert!(cy.contains('y'), "got {cy}");
        }
        other => panic!("expected Gradient, got {other:?}"),
    }
}

#[test]
fn jacobian_linear_map() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let out = run(
        &engine,
        &h,
        CalculusRequest::Jacobian {
            expressions: vec![h.ap("Plus", vec![h.sym("x"), h.sym("y")]), h.sym("x")],
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Jacobian(j), .. } => {
            assert_eq!(j.rows.len(), 2);
            assert_eq!(h.dbg(j.rows[0][0]), "1");
            assert_eq!(h.dbg(j.rows[0][1]), "1");
            assert_eq!(h.dbg(j.rows[1][0]), "1");
            assert_eq!(h.dbg(j.rows[1][1]), "0");
        }
        other => panic!("expected Jacobian, got {other:?}"),
    }
}

#[test]
fn hessian_quadratic() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap(
        "Plus",
        vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.ap("Times", vec![h.sym("x"), h.sym("y")])],
    );
    let out = run(
        &engine,
        &h,
        CalculusRequest::Hessian {
            expression: expr,
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Hessian(hh), .. } => {
            assert_eq!(hh.entries.len(), 2);
            assert_eq!(h.dbg(hh.entries[0][0]), "2");
            assert_eq!(h.dbg(hh.entries[0][1]), "1");
            assert_eq!(h.dbg(hh.entries[1][0]), "1");
            assert_eq!(h.dbg(hh.entries[1][1]), "0");
        }
        other => panic!("expected Hessian, got {other:?}"),
    }
}

#[test]
fn divergence_of_linear_field() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // 场：F = (x, y) ⇒ div = 2
    let out = run(
        &engine,
        &h,
        CalculusRequest::Divergence {
            components: vec![h.sym("x"), h.sym("y")],
            variables: vec!["x".into(), "y".into()],
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Divergence(d), .. } => {
            assert_eq!(h.dbg(d.value), "2");
        }
        other => panic!("expected Divergence, got {other:?}"),
    }
}

#[test]
fn curl_of_linear_3d_field() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // 场：F = (−y, x, 0) ⇒ curl = (0, 0, 2)
    let out = run(
        &engine,
        &h,
        CalculusRequest::Curl {
            components: vec![h.ap("Times", vec![h.i(-1), h.sym("y")]), h.sym("x"), h.i(0)],
            variables: vec!["x".into(), "y".into(), "z".into()],
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Curl(c), .. } => {
            assert_eq!(
                c.curl_components.iter().map(|t| h.dbg(*t)).collect::<Vec<_>>(),
                vec!["0", "0", "2"]
            );
        }
        other => panic!("expected Curl, got {other:?}"),
    }
}

#[test]
fn divergence_via_term_lowering() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let term = h.ap(
        "Divergence",
        vec![
            h.lst(vec![h.sym("x"), h.sym("y"), h.sym("z")]),
            h.lst(vec![h.sym("x"), h.sym("y"), h.sym("z")]),
        ],
    );
    let req = lower(&h, term);
    let out = run(&engine, &h, req);
    match out {
        CalculusResult::Exact { value: CalculusValue::Divergence(d), .. } => {
            assert_eq!(h.dbg(d.value), "3");
        }
        other => panic!("expected Divergence, got {other:?}"),
    }
}

#[test]
fn ode_y_prime_equals_const() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let eq = h.ap("Equal", vec![h.ap("D", vec![h.sym("y"), h.sym("x")]), h.i(2)]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, VerificationStatus::Verified { .. }));
            assert_eq!(h.dbg(sol.explicit), "Times[2, x]");
        }
        other => panic!("expected verified ODE solution, got {other:?}"),
    }
}

#[test]
fn ode_ivp_y_prime_const() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let eq = h.ap("Equal", vec![h.ap("D", vec![h.sym("y"), h.sym("x")]), h.i(2)]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: Some((h.i(0), h.i(1))),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, VerificationStatus::Verified { .. }));
            // 解：y = 2x + 1
            let text = h.dbg(sol.explicit);
            assert!(text.contains('x') && text.contains('1'), "got {text}");
        }
        other => panic!("expected IVP solution, got {other:?}"),
    }
}

#[test]
fn ode_separable_g_of_x() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // ODE：y' = x ⇒ y = x²/2
    let eq = h.ap("Equal", vec![h.ap("D", vec![h.sym("y"), h.sym("x")]), h.sym("x")]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, VerificationStatus::Verified { .. }));
            let text = h.dbg(sol.explicit);
            assert!(text.contains('x'), "got {text}");
        }
        other => panic!("expected separable g(x) solution, got {other:?}"),
    }
}

#[test]
fn ode_power_y_squared() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // ODE：y' = y² ⇒ y = -1/x
    let eq = h.ap(
        "Equal",
        vec![h.ap("D", vec![h.sym("y"), h.sym("x")]), h.ap("Power", vec![h.sym("y"), h.i(2)])],
    );
    let out = run(
        &engine,
        &h,
        CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, VerificationStatus::Verified { .. }));
            assert_eq!(h.dbg(sol.explicit), "Times[-1, Power[x, -1]]");
        }
        other => panic!("expected y=-1/x, got {other:?}"),
    }
}

#[test]
fn ode_bernoulli_const_and_separable_xy2() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // ODE：y' = 2y + y² ⇒ y = -2
    let eq = h.ap(
        "Equal",
        vec![
            h.ap("D", vec![h.sym("y"), h.sym("x")]),
            h.ap(
                "Plus",
                vec![h.ap("Times", vec![h.i(2), h.sym("y")]), h.ap("Power", vec![h.sym("y"), h.i(2)])],
            ),
        ],
    );
    let out = run(
        &engine,
        &h,
        CalculusRequest::SolveOde {
            equation: eq,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, VerificationStatus::Verified { .. }));
            assert_eq!(h.dbg(sol.explicit), "-2");
        }
        other => panic!("expected Bernoulli constant y=-2, got {other:?}"),
    }

    // ODE：y' = x·y² ⇒ y = -1/(x²/2) = -2/x²
    let eq2 = h.ap(
        "Equal",
        vec![
            h.ap("D", vec![h.sym("y"), h.sym("x")]),
            h.ap("Times", vec![h.sym("x"), h.ap("Power", vec![h.sym("y"), h.i(2)])]),
        ],
    );
    let out2 = run(
        &engine,
        &h,
        CalculusRequest::SolveOde {
            equation: eq2,
            dependent: "y".into(),
            independent: "x".into(),
            initial: None,
            assumptions: AssumptionSet::empty(),
        },
    );
    match out2 {
        CalculusResult::Exact { value: CalculusValue::DifferentialSolution(sol), .. } => {
            assert!(matches!(sol.verified, VerificationStatus::Verified { .. }));
            let text = h.dbg(sol.explicit);
            assert!(text.contains('x'), "got {text}");
        }
        other => panic!("expected separable x y^2 solution, got {other:?}"),
    }
}

#[test]
fn laplace_exp_and_sin() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let out = run(
        &engine,
        &h,
        CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Laplace,
            expression: h.ap("Exp", vec![h.ap("Times", vec![h.i(2), h.sym("t")])]),
            time_variable: "t".into(),
            transform_variable: "s".into(),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            let text = h.dbg(tr.expression);
            assert!(text.contains('s'), "got {text}");
        }
        other => panic!("expected Laplace Transform, got {other:?}"),
    }

    let sin = run(
        &engine,
        &h,
        CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Laplace,
            expression: h.ap("Sin", vec![h.sym("t")]),
            time_variable: "t".into(),
            transform_variable: "s".into(),
            assumptions: AssumptionSet::empty(),
        },
    );
    assert!(matches!(sin, CalculusResult::Exact { value: CalculusValue::Transform(_), .. }));
}

#[test]
fn fourier_exp_abs_decay() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // 形态：Exp[-Abs[t]] → 2/(1+ω²)
    let expr = h.ap("Exp", vec![h.ap("Times", vec![h.i(-1), h.ap("Abs", vec![h.sym("t")])])]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Fourier,
            expression: expr,
            time_variable: "t".into(),
            transform_variable: "w".into(),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            assert_eq!(tr.kind, athena_engine::TransformKind::Fourier);
            let text = h.dbg(tr.expression);
            assert!(text.contains('w'), "got {text}");
            // 2 / (1 + w^2)
            assert_eq!(h.dbg(tr.expression), "Times[2, Power[Plus[1, Power[w, 2]], -1]]");
        }
        other => panic!("expected Fourier Transform, got {other:?}"),
    }
}

#[test]
fn fourier_gaussian_and_lowering() {
    let engine = AthenaEngine::new();
    let h = H::new();
    let expr = h.ap(
        "Exp",
        vec![h.ap("Times", vec![h.i(-1), h.ap("Power", vec![h.sym("t"), h.i(2)])])],
    );
    let term = h.ap("FourierTransform", vec![expr, h.sym("t"), h.sym("w")]);
    let req = lower(&h, term);
    let out = run(&engine, &h, req);
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            let text = h.dbg(tr.expression);
            assert!(text.contains("Sqrt") || text.contains("Pi") || text.contains('w'), "got {text}");
        }
        other => panic!("expected Fourier Transform, got {other:?}"),
    }

    let causal = h.ap(
        "Times",
        vec![
            h.ap("UnitStep", vec![h.sym("t")]),
            h.ap("Exp", vec![h.ap("Times", vec![h.i(-2), h.sym("t")])]),
        ],
    );
    let causal_out = run(
        &engine,
        &h,
        CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Fourier,
            expression: causal,
            time_variable: "t".into(),
            transform_variable: "w".into(),
            assumptions: AssumptionSet::empty(),
        },
    );
    match causal_out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            let text = h.dbg(tr.expression);
            assert!(text.contains('I') && text.contains('w'), "got {text}");
        }
        other => panic!("expected causal Fourier, got {other:?}"),
    }
}

#[test]
fn z_transform_geometric_and_delta() {
    let engine = AthenaEngine::new();
    let h = H::new();
    // 2^n → z/(z-2), |z|>2
    let geom = h.ap("Power", vec![h.i(2), h.sym("n")]);
    let out = run(
        &engine,
        &h,
        CalculusRequest::Transform {
            kind: athena_engine::TransformKind::Z,
            expression: geom,
            time_variable: "n".into(),
            transform_variable: "z".into(),
            assumptions: AssumptionSet::empty(),
        },
    );
    match out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert!(tr.region_of_convergence.known);
            assert_eq!(tr.kind, athena_engine::TransformKind::Z);
            let text = h.dbg(tr.expression);
            assert!(text.contains('z'), "got {text}");
            let roc = tr
                .region_of_convergence
                .predicate
                .map(|p| h.dbg(p))
                .unwrap_or_default();
            assert!(roc.contains("Abs"), "got {roc}");
        }
        other => panic!("expected Z Transform, got {other:?}"),
    }

    let delta = h.ap("KroneckerDelta", vec![h.sym("n")]);
    let term = h.ap("ZTransform", vec![delta, h.sym("n"), h.sym("z")]);
    let req = lower(&h, term);
    let delta_out = run(&engine, &h, req);
    match delta_out {
        CalculusResult::Exact { value: CalculusValue::Transform(tr), .. } => {
            assert_eq!(h.dbg(tr.expression), "1");
            assert!(tr.region_of_convergence.known);
        }
        other => panic!("expected delta Z, got {other:?}"),
    }
}

#[test]
fn try_calculus_request_d_limit_series() {
    let h = H::new();
    let d = h.ap("D", vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.sym("x")]);
    assert!(h.with_cc(|cc| matches!(try_calculus_request(cc, d), Some(CalculusRequest::Derivative { .. }))));
    let lim = h.ap("Limit", vec![h.sym("x"), h.ap("Rule", vec![h.sym("x"), h.i(1)])]);
    assert!(h.with_cc(|cc| matches!(try_calculus_request(cc, lim), Some(CalculusRequest::Limit { .. }))));
    let series = h.ap(
        "Series",
        vec![h.ap("Power", vec![h.sym("x"), h.i(2)]), h.lst(vec![h.sym("x"), h.i(0), h.i(3)])],
    );
    assert!(h.with_cc(|cc| matches!(try_calculus_request(cc, series), Some(CalculusRequest::Series { .. }))));
}

#[test]
fn evaluate_routes_d_through_domain() {
    let h = H::new();
    let x = h.sym("x");
    let three = h.i(3);
    let cube = h.ap("Power", vec![x, three]);
    let d = h.ap("D", vec![cube, x]);
    let out = h.with_session(|s| athena_engine::interp::vm::evaluate_session(s, d));
    let text = h.dbg(out.term);
    assert!(text.contains('x'), "got {text}");
}
