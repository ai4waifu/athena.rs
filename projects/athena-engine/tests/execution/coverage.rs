//! 执行层覆盖（Living `25` L2 · `evaluate_term` → `ExecutionIR`）。

use athena_engine::{
    api::request::{AthenaRequest, ControlPlan},
    diagnostics::term_summary::term_debug,
    execution,
    execution::evaluate_term,
    runtime::{
        Session,
        values::arena::{push_constant, push_extension, push_int, push_list, push_semantic, push_symbol_name},
    },
};
use athena_types::BindingEvaluationPolicy;
use athena_ir::{MathematicalConstant, SemanticOperator, UnaryFunction};

type Tid = athena_types::TermId;

struct C {
    s: Session,
}

impl C {
    fn new() -> Self {
        Self { s: Session::new() }
    }
}

fn out(e: Tid, c: &mut C) -> execution::TermEvaluation {
    evaluate_term(&mut c.s, e)
}

/// 求值并渲染为调试串。
fn t(e: Tid, c: &mut C) -> String {
    let o = evaluate_term(&mut c.s, e);
    term_debug(&c.s, o.term)
}

fn symbol(name: &str, c: &mut C) -> Tid {
    push_symbol_name(&mut c.s, name)
}

fn math_const(value: MathematicalConstant, c: &mut C) -> Tid {
    push_constant(&mut c.s, value)
}

fn i(n: i64, c: &mut C) -> Tid {
    push_int(&mut c.s, n)
}

fn str_(v: &str, c: &mut C) -> Tid {
    let span = athena_ir::TermNode::default_span();
    c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::String(v.into())), span)
}

fn boolean(v: bool, c: &mut C) -> Tid {
    let span = athena_ir::TermNode::default_span();
    c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(v)), span)
}

fn lst(items: Vec<Tid>, c: &mut C) -> Tid {
    push_list(&mut c.s, items)
}

fn sem(op: SemanticOperator, args: Vec<Tid>, c: &mut C) -> Tid {
    push_semantic(&mut c.s, op, args)
}

fn unary(f: UnaryFunction, args: Vec<Tid>, c: &mut C) -> Tid {
    push_semantic(&mut c.s, SemanticOperator::from_unary(f), args)
}

fn ext(head: &str, args: Vec<Tid>, c: &mut C) -> Tid {
    let op = c.s.operators.intern(head);
    push_extension(&mut c.s, op, args)
}

#[test]
fn plus_fold() {
    let mut c = C::new();
    let x = symbol("x", &mut c);
    let e = sem(SemanticOperator::Add, vec![i(1, &mut c), i(2, &mut c), x], &mut c);
    assert_eq!(t(e, &mut c), "Add[3, x]");
}

#[test]
fn power_one() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Power, vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "x");
}

#[test]
fn list_eval() {
    let mut c = C::new();
    let inner = sem(SemanticOperator::Add, vec![i(2, &mut c), i(2, &mut c)], &mut c);
    let e = lst(vec![i(1, &mut c), inner], &mut c);
    assert_eq!(t(e, &mut c), "OrderedCollection[1, 4]");
}

#[test]
fn pythagorean() {
    let mut c = C::new();
    let sin2 = sem(SemanticOperator::Power, vec![unary(UnaryFunction::Sin, vec![symbol("x", &mut c)], &mut c), i(2, &mut c)], &mut c);
    let cos2 = sem(SemanticOperator::Power, vec![unary(UnaryFunction::Cos, vec![symbol("x", &mut c)], &mut c), i(2, &mut c)], &mut c);
    let e = sem(SemanticOperator::Simplify, vec![sem(SemanticOperator::Add, vec![sin2, cos2], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "1");
}

#[test]
fn map_sin_list() {
    let mut c = C::new();
    // Living `27`: Map over a 0-ary semantic operator value, not a display-name symbol.
    let sin = sem(SemanticOperator::from_unary(UnaryFunction::Sin), vec![], &mut c);
    let e = sem(SemanticOperator::Map, vec![sin, lst(vec![i(0, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "OrderedCollection[0]");
}

#[test]
fn truthy_via_and_or() {
    let mut c = C::new();
    assert_eq!(t(sem(SemanticOperator::And, vec![i(0, &mut c), i(1, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(sem(SemanticOperator::And, vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
    assert_eq!(t(sem(SemanticOperator::Or, vec![i(0, &mut c), i(0, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(sem(SemanticOperator::Or, vec![i(0, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
    assert_eq!(t(sem(SemanticOperator::And, vec![boolean(true, &mut c), boolean(false, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(sem(SemanticOperator::Not, vec![boolean(true, &mut c)], &mut c), &mut c), "False");
}



#[test]
fn unsupported_import_is_not_silent_value() {
    use athena_types::ComputationStatus;
    let mut c = C::new();
    // Import is an Extension residual until a typed SessionCommand/Goal exists (Living 27).
    let e = ext("Import", vec![str_("x.csv", &mut c)], &mut c);
    let o = out(e, &mut c);
    assert_eq!(o.kind, execution::EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert!(!o.has_error());
}

#[test]
fn unknown_head_is_unevaluated_not_exact_value() {
    use athena_types::ComputationStatus;
    let mut c = C::new();
    let e = ext("FooBar", vec![i(1, &mut c)], &mut c);
    let o = out(e, &mut c);
    assert_eq!(o.kind, execution::EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert!(!o.has_error());
}

#[test]
fn typed_boolean_and_null_atoms_render_and_symbols_stay_symbols() {
    let mut c = C::new();
    assert_eq!(t(boolean(true, &mut c), &mut c), "True");
    assert_eq!(t(boolean(false, &mut c), &mut c), "False");
    {
        let span = athena_ir::TermNode::default_span();
        let null = c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::Null), span);
        assert_eq!(t(null, &mut c), "Null");
    }
    // Display-name symbols must not canonicalize into typed boolean/null atoms.
    let true_sym = symbol("True", &mut c);
    assert_eq!(t(true_sym, &mut c), "True");
    assert!(matches!(
        c.s.arena.get(true_sym),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(_)))
    ));
    assert_eq!(t(sem(SemanticOperator::Equal, vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
}

#[test]
fn hold_and_hold_form_do_not_eval_args() {
    let mut c = C::new();
    assert_eq!(t(sem(SemanticOperator::Hold, vec![sem(SemanticOperator::Add, vec![i(1, &mut c), i(1, &mut c)], &mut c)], &mut c), &mut c), "Hold[Add[1, 1]]");
    assert_eq!(t(sem(SemanticOperator::Hold, vec![sem(SemanticOperator::Add, vec![i(2, &mut c), i(3, &mut c)], &mut c)], &mut c), &mut c), "Hold[Add[2, 3]]");
}

#[test]
fn cond_picks_first_true_branch() {
    let mut c = C::new();
    let request = AthenaRequest::Control(ControlPlan::Cond {
        arms: vec![
            (boolean(false, &mut c), Box::new(AthenaRequest::Term(i(1, &mut c)))),
            (boolean(true, &mut c), Box::new(AthenaRequest::Term(i(2, &mut c)))),
            (boolean(true, &mut c), Box::new(AthenaRequest::Term(i(3, &mut c)))),
        ],
        otherwise: None,
    });
    let result_id = execution::execute_ir_request(&mut c.s, request).expect("cond");
    let term = c.s.results.get(result_id).and_then(|r| r.symbolic_term).expect("term");
    assert_eq!(term_debug(&c.s, term), "2");
}



#[test]
fn compare_chain_less_expands_to_and() {
    let mut c = C::new();
    let nested = sem(SemanticOperator::Less, vec![i(1, &mut c), i(2, &mut c)], &mut c);
    let e = sem(SemanticOperator::Less, vec![nested, i(3, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "True");
    let nested = sem(SemanticOperator::Less, vec![i(1, &mut c), i(0, &mut c)], &mut c);
    let e2 = sem(SemanticOperator::Less, vec![nested, i(3, &mut c)], &mut c);
    assert_eq!(t(e2, &mut c), "False");
}

#[test]
fn session_setdelayed_evaluates_on_use() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::execution::{compiler::ExecutionCompiler, reference::ReferenceExecutor};
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut c = C::new();
    let sym = symbol("a", &mut c);
    let symbol_id = match c.s.arena.get(sym) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let rhs = sem(SemanticOperator::Add, vec![i(1, &mut c), i(1, &mut c)], &mut c);
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol: symbol_id,
        value: rhs,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::StoreResidualTerm,
    });
    let module = ExecutionCompiler::new().compile(&mut c.s, &request).expect("define residual");
    ReferenceExecutor::new().execute(&mut c.s, &module, None).expect("exec");
    assert_eq!(t(symbol("a", &mut c), &mut c), "2");
}

#[test]
fn session_pattern_dispatch_rule() {
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{Atom, TermNode};
    let mut c = C::new();
    let x_term = symbol("x", &mut c);
    let x_sym = match c.s.arena.get(x_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let f_op = c.s.operators.intern("f");
    let rhs = sem(SemanticOperator::Power, vec![symbol("x", &mut c), i(2, &mut c)], &mut c);
    c.s.defs.register_extension_rule(
        f_op,
        TermPattern::Application {
            operator: athena_ir::ApplicationHead::Extension(f_op),
            arguments: vec![TermPattern::Bind { name: x_sym, inner: Box::new(TermPattern::Any) }],
        },
        rhs,
    );
    assert_eq!(t(ext("f", vec![i(3, &mut c)], &mut c), &mut c), "9");
}

#[test]
fn compare_list_scalar_broadcasts() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Less, vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), i(2, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "OrderedCollection[True, False, False]");
}

#[test]
fn dispatch_literal_then_bind_fallback() {
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{Atom, TermNode};
    let mut c = C::new();
    let f_op = c.s.operators.intern("f");
    let one = i(1, &mut c);
    let ten = i(10, &mut c);
    c.s.defs.register_extension_rule(
        f_op,
        TermPattern::Application {
            operator: athena_ir::ApplicationHead::Extension(f_op),
            arguments: vec![TermPattern::Exact(one)],
        },
        ten,
    );
    let x_term = symbol("x", &mut c);
    let x_sym = match c.s.arena.get(x_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let rhs2 = sem(SemanticOperator::Multiply, vec![symbol("x", &mut c), i(2, &mut c)], &mut c);
    c.s.defs.register_extension_rule(
        f_op,
        TermPattern::Application {
            operator: athena_ir::ApplicationHead::Extension(f_op),
            arguments: vec![TermPattern::Bind { name: x_sym, inner: Box::new(TermPattern::Any) }],
        },
        rhs2,
    );
    assert_eq!(t(ext("f", vec![i(1, &mut c)], &mut c), &mut c), "10");
    assert_eq!(t(ext("f", vec![i(5, &mut c)], &mut c), &mut c), "10");
}

#[test]
fn replace_all_literal() {
    let mut c = C::new();
    let rule = sem(SemanticOperator::Rule, vec![symbol("x", &mut c), i(9, &mut c)], &mut c);
    let e = sem(SemanticOperator::ReplaceAll, vec![sem(SemanticOperator::Add, vec![symbol("x", &mut c), i(1, &mut c)], &mut c), rule], &mut c);
    assert_eq!(t(e, &mut c), "10");
}

#[test]
fn apply_and_join_and_length() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Apply, vec![sem(SemanticOperator::Add, vec![], &mut c), lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    let e = sem(SemanticOperator::Join, vec![lst(vec![i(1, &mut c)], &mut c), lst(vec![i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "OrderedCollection[1, 2, 3]");
    let e = sem(SemanticOperator::Length, vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn cases_filters_by_value_type_pattern() {
    use athena_engine::execution::builtins::patterns::match_term_pattern;
    use athena_engine::reasoning::trs::{PatternConstraint, TermPattern};
    use athena_types::ValueTypeId;
    let mut c = C::new();
    let pat = TermPattern::Constrained {
        pattern: Box::new(TermPattern::Any),
        constraint: PatternConstraint::ValueType(ValueTypeId::ExactInteger),
    };
    let items = [i(1, &mut c), symbol("y", &mut c), i(3, &mut c)];
    let mut out = Vec::new();
    for item in items {
        let mut binds = std::collections::HashMap::new();
        if match_term_pattern(&c.s, item, &pat, &mut binds) {
            out.push(item);
        }
    }
    assert_eq!(out.len(), 2);
    assert_eq!(t(out[0], &mut c), "1");
    assert_eq!(t(out[1], &mut c), "3");
}

#[test]
fn array_sum_vector_and_matrix() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Sum, vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
}

#[test]
fn det_and_size() {
    let mut c = C::new();
    let m = lst(vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    assert_eq!(t(sem(SemanticOperator::Determinant, vec![m.clone()], &mut c), &mut c), "-2");
    let r = t(sem(SemanticOperator::Size, vec![m], &mut c), &mut c);
    assert!(r.contains("OrderedCollection[2, 2]"), "got {r}");
}

#[test]
fn linear_solve_via_domain_goal() {
    use athena_engine::{
        api::request::DomainGoal,
        domains::{
            dispatch::{DomainRequest, DomainResult},
            linear_algebra::{
                ExactSolveResult, LinearAlgebraRequest, LinearAlgebraResult, LinearAlgebraValue, MatrixEqualityKind, MatrixValue,
                SolveDisposition, matrices_equal,
            },
        },
        runtime::values::RuntimeValue,
    };
    use athena_numeric::{Integer, Rational};

    let mut c = C::new();
    let a = c.s.matrix_objects.intern(
        MatrixValue::from_integers_row_major(
            2,
            2,
            vec![Integer::from_i64(2), Integer::from_i64(0), Integer::from_i64(0), Integer::from_i64(2)],
        )
        .unwrap(),
    );
    let b = c
        .s
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 1, vec![Integer::from_i64(4), Integer::from_i64(6)]).unwrap());
    let expected = MatrixValue::from_rationals_row_major(
        2,
        1,
        vec![Rational::new(Integer::from_i64(2), Integer::from_i64(1)), Rational::new(Integer::from_i64(3), Integer::from_i64(1))],
    )
    .unwrap();
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Solve { a, b })));
    let result_id = execution::execute_ir_request(&mut c.s, request).expect("goal");
    let loaded = c.s.results.get(result_id).expect("result");
    let value_id = loaded.value.expect("value");
    match c.s.values.get(value_id).expect("runtime") {
        RuntimeValue::Domain(DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
            value: LinearAlgebraValue::ExactSolve(ExactSolveResult {
                disposition: SolveDisposition::Unique,
                particular: Some(x),
                ..
            }),
        })) => assert!(matrices_equal(x, &expected, MatrixEqualityKind::ExactMathematical).unwrap()),
        other => panic!("expected ExactSolve unique, got {other:?}"),
    }
}

#[test]
fn machine_trig_at_real_points() {
    let mut c = C::new();
    // Sin[0.0] → 精确 0；Cos[Pi] → -1。
    let zero = {
        let span = athena_ir::TermNode::default_span();
        c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::Number(athena_numeric::NumericValue::machine(0.0))), span)
    };
    let e = unary(UnaryFunction::Sin, vec![zero], &mut c);
    assert_eq!(t(e, &mut c), "0");
    let e = unary(UnaryFunction::Cos, vec![math_const(MathematicalConstant::Pi, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "-1");
}

#[test]
fn sum_over_iterator_folds() {
    let mut c = C::new();
    let iter = lst(vec![symbol("k", &mut c), i(1, &mut c), i(4, &mut c)], &mut c);
    let e = sem(SemanticOperator::Sum, vec![sem(SemanticOperator::Power, vec![symbol("k", &mut c), i(2, &mut c)], &mut c), iter], &mut c);
    assert_eq!(t(e, &mut c), "30");
}

#[test]
fn iterate_with_explicit_range() {
    let mut c = C::new();
    let binder = symbol("i", &mut c);
    let range = lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c);
    let request = AthenaRequest::Control(ControlPlan::Iterate {
        binder,
        range,
        body: Box::new(AthenaRequest::Term(binder)),
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let result_id = execution::execute_ir_request(&mut c.s, request).expect("ir");
    let term = c.s.results.get(result_id).and_then(|r| r.symbolic_term).expect("term");
    assert_eq!(term_debug(&c.s, term), "OrderedCollection[1, 2, 3]");
}
