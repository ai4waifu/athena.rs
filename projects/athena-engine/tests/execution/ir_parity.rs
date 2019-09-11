//! `ExecutionIR` reference parity (Living `25` L2).

use athena_engine::{
    api::request::{AthenaRequest, ControlPlan},
    diagnostics::term_summary::term_debug,
    execution::execute_ir_request,
    runtime::{
        Session,
        results::CoverageStatus,
        values::arena::{push_application_named, push_int, push_list, push_semantic, push_symbol_name},
    },
};
use athena_types::{BindingEvaluationPolicy, ComputationStatus, TermId};
use athena_ir::{SemanticOperator, UnaryFunction};

type Tid = TermId;

struct C {
    s: Session,
}

impl C {
    fn new() -> Self {
        Self { s: Session::new() }
    }
}

fn t(e: Tid, c: &mut C) -> String {
    let out = c.s.evaluate(e);
    term_debug(&c.s, out)
}

fn result_of<'a>(e: Tid, c: &'a mut C) -> &'a athena_engine::runtime::results::ComputationResult {
    let id = execute_ir_request(&mut c.s, AthenaRequest::Term(e)).expect("ir");
    c.s.results.get(id).expect("result")
}

fn symbol(name: &str, c: &mut C) -> Tid {
    push_symbol_name(&mut c.s, name)
}

fn i(n: i64, c: &mut C) -> Tid {
    push_int(&mut c.s, n)
}

fn boolean(v: bool, c: &mut C) -> Tid {
    let span = athena_ir::TermNode::default_span();
    c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(v)), span)
}

fn str_(v: &str, c: &mut C) -> Tid {
    let span = athena_ir::TermNode::default_span();
    c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::String(v.into())), span)
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
    push_application_named(&mut c.s, head, args)
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
    assert_eq!(t(e, &mut c), "List[1, 4]");
}

#[test]
fn compound_expression_returns_last() {
    let mut c = C::new();
    let e = ext("Sequence", vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
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
fn unknown_head_is_unevaluated_not_exact_value() {
    let mut c = C::new();
    let e = ext("FooBar", vec![i(1, &mut c)], &mut c);
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert_eq!(o.coverage, CoverageStatus::Partial);
    assert!(o.diagnostics.is_empty());
}







#[test]
fn while_false_skips_body() {
    let mut c = C::new();
    let e = ext("LoopWhile", vec![i(0, &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "Null");
}

#[test]
fn compound_set_binds_for_later_stmts() {
    let mut c = C::new();
    let set = ext("Define", vec![symbol("x", &mut c), i(5, &mut c)], &mut c);
    let e = ext("Sequence", vec![set, sem(SemanticOperator::Add, vec![symbol("x", &mut c), i(1, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
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
fn with_module_block_local_bindings() {
    let locals = |c: &mut C| {
        let l = lst(vec![ext("Define", vec![symbol("x", c), i(1, c)], c)], c);
        let b = sem(SemanticOperator::Add, vec![symbol("x", c), i(1, c)], c);
        (l, b)
    };
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(ext("LocalScope", vec![l, b], &mut d), &mut d), "2");
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(ext("LexicalScope", vec![l, b], &mut d), &mut d), "2");
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(ext("DynamicScope", vec![l, b], &mut d), &mut d), "2");
}

#[test]
fn module_bare_local_is_renamed_unique() {
    let mut c = C::new();
    let e1 = ext("LexicalScope", vec![lst(vec![symbol("x", &mut c)], &mut c), symbol("x", &mut c)], &mut c);
    let r1 = t(e1, &mut c);
    let e2 = ext("LexicalScope", vec![lst(vec![symbol("x", &mut c)], &mut c), symbol("x", &mut c)], &mut c);
    let r2 = t(e2, &mut c);
    assert!(r1.starts_with("x$"), "got {r1}");
    assert!(r2.starts_with("x$"), "got {r2}");
    assert_ne!(r1, r2);
}

#[test]
fn try_catch_on_error_and_success() {
    let mut c = C::new();
    let err = ext("Recover", vec![ext("error", vec![str_("e", &mut c)], &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(err, &mut c), "1");
    let ok = ext("Recover", vec![i(2, &mut c), i(3, &mut c)], &mut c);
    assert_eq!(t(ok, &mut c), "2");
}

#[test]
fn session_set_persists_across_evaluate() {
    let mut c = C::new();
    let set = ext("Define", vec![symbol("x", &mut c), i(5, &mut c)], &mut c);
    assert_eq!(t(set, &mut c), "5");
    let e = sem(SemanticOperator::Add, vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    let mut d = C::new();
    let e = sem(SemanticOperator::Add, vec![symbol("x", &mut d), i(1, &mut d)], &mut d);
    let r = t(e, &mut d);
    assert!(r.contains("x"), "expected free x, got {r}");
}

#[test]
fn map_sin_list() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Map, vec![symbol("Sin", &mut c), lst(vec![i(0, &mut c)], &mut c)], &mut c);
    let r = t(e, &mut c);
    assert!(r.starts_with("List["), "got {r}");
}

#[test]
fn map_function_var_body() {
    let mut c = C::new();
    let body = sem(SemanticOperator::Add, vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    let f = sem(SemanticOperator::Function, vec![symbol("x", &mut c), body], &mut c);
    let e = sem(SemanticOperator::Map, vec![f, lst(vec![i(1, &mut c), i(2, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[2, 3]");
}

#[test]
fn map_named_function_binder() {
    let mut c = C::new();
    let body = sem(SemanticOperator::Multiply, vec![symbol("x", &mut c), i(2, &mut c)], &mut c);
    let f = sem(SemanticOperator::Function, vec![symbol("x", &mut c), body], &mut c);
    let e = sem(SemanticOperator::Map, vec![f, lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[6, 8]");
}

#[test]
fn application_named_function_binder() {
    let mut c = C::new();
    let body = sem(SemanticOperator::Add, vec![symbol("x", &mut c), i(10, &mut c)], &mut c);
    let f = sem(SemanticOperator::Function, vec![symbol("x", &mut c), body], &mut c);
    let e = sem(SemanticOperator::ApplyHead, vec![f, i(7, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "17");
}

#[test]
fn for_range_last_value() {
    let mut c = C::new();
    let e =
        ext("CountedLoop", vec![symbol("i", &mut c), sem(SemanticOperator::Range, vec![i(1, &mut c), i(3, &mut c)], &mut c), symbol("i", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn for_accumulator_shares_compound_bindings() {
    let mut c = C::new();
    let set0 = ext("Define", vec![symbol("s", &mut c), i(0, &mut c)], &mut c);
    let body = ext("Define", vec![symbol("s", &mut c), sem(SemanticOperator::Add, vec![symbol("s", &mut c), symbol("i", &mut c)], &mut c)], &mut c);
    let f = ext("CountedLoop", vec![symbol("i", &mut c), sem(SemanticOperator::Range, vec![i(1, &mut c), i(3, &mut c)], &mut c), body], &mut c);
    let e = ext("Sequence", vec![set0, f, symbol("s", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
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
fn compare_list_scalar_broadcasts() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Less, vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), i(2, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[True, False, False]");
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
    assert_eq!(t(e, &mut c), "List[1, 2, 3]");
    let e = sem(SemanticOperator::Length, vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn module_local_does_not_clobber_session() {
    let mut c = C::new();
    let set = ext("Define", vec![symbol("x", &mut c), i(5, &mut c)], &mut c);
    let _ = c.s.evaluate(set);
    let locals = lst(vec![ext("Define", vec![symbol("x", &mut c), i(1, &mut c)], &mut c)], &mut c);
    let body = sem(SemanticOperator::Add, vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    let e = ext("LexicalScope", vec![locals, body], &mut c);
    assert_eq!(t(e, &mut c), "2");
    assert_eq!(t(symbol("x", &mut c), &mut c), "5");
}

#[test]
fn nested_module_names_do_not_collide() {
    let mut c = C::new();
    let inner_locals = lst(vec![symbol("x", &mut c)], &mut c);
    let inner = ext("LexicalScope", vec![inner_locals, symbol("x", &mut c)], &mut c);
    let outer_locals = lst(vec![symbol("x", &mut c)], &mut c);
    let e = ext("LexicalScope", vec![outer_locals, inner], &mut c);
    let r = t(e, &mut c);
    assert!(r.starts_with("x$"), "got {r}");
}

#[test]
fn array_sum_vector_and_matrix() {
    let mut c = C::new();
    let e = sem(SemanticOperator::Sum, vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    let m = lst(vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    assert_eq!(t(sem(SemanticOperator::Sum, vec![m], &mut c), &mut c), "List[4, 6]");
}

#[test]
fn size_of_matrix() {
    let mut c = C::new();
    let m = lst(vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    let r = t(sem(SemanticOperator::Size, vec![m], &mut c), &mut c);
    assert!(r.contains("List[2, 2]"), "got {r}");
}

#[test]
fn det_and_size() {
    let mut c = C::new();
    let m = lst(vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    assert_eq!(t(sem(SemanticOperator::Determinant, vec![m.clone()], &mut c), &mut c), "-2");
    let r = t(sem(SemanticOperator::Size, vec![m], &mut c), &mut c);
    assert!(r.contains("List[2, 2]"), "got {r}");
}

#[test]
fn linear_solve_column_vector() {
    let mut c = C::new();
    let m = lst(vec![lst(vec![i(2, &mut c), i(0, &mut c)], &mut c), lst(vec![i(0, &mut c), i(2, &mut c)], &mut c)], &mut c);
    let b = lst(vec![lst(vec![i(4, &mut c)], &mut c), lst(vec![i(6, &mut c)], &mut c)], &mut c);
    let r = t(ext("LinearSolve", vec![m, b], &mut c), &mut c);
    assert!(r.contains("List[2]"), "got {r}");
}

#[test]
fn hold_and_hold_form_do_not_eval_args() {
    let mut c = C::new();
    assert_eq!(t(sem(SemanticOperator::Hold, vec![sem(SemanticOperator::Add, vec![i(1, &mut c), i(1, &mut c)], &mut c)], &mut c), &mut c), "Hold[Add[1, 1]]");
}

#[test]
fn symbol_true_false_null_canonicalize() {
    let mut c = C::new();
    assert_eq!(t(symbol("True", &mut c), &mut c), "True");
    assert_eq!(t(symbol("False", &mut c), &mut c), "False");
    assert_eq!(t(symbol("Null", &mut c), &mut c), "Null");
    assert_eq!(t(sem(SemanticOperator::Equal, vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
}

#[test]
fn if_true_branch() {
    let mut c = C::new();
    let cond = sem(SemanticOperator::Equal, vec![i(1, &mut c), i(1, &mut c)], &mut c);
    let e = ext("Branch", vec![cond, i(7, &mut c), i(8, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "7");
}

#[test]
fn cond_picks_first_true_branch() {
    let mut c = C::new();
    let e = ext(
        "Cond",
        vec![symbol("False", &mut c), i(1, &mut c), symbol("True", &mut c), i(2, &mut c), symbol("True", &mut c), i(3, &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "2");
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
    let f_sym = c.s.arena.symbols_mut().intern("f");
    let rhs = sem(SemanticOperator::Power, vec![symbol("x", &mut c), i(2, &mut c)], &mut c);
    c.s.defs.register_rule(
        f_sym,
        TermPattern::Application {
            operator: athena_ir::ApplicationHead::Extension(f_op),
            arguments: vec![TermPattern::Bind { name: x_sym, inner: Box::new(TermPattern::Any) }],
        },
        rhs,
    );
    assert_eq!(t(ext("f", vec![i(3, &mut c)], &mut c), &mut c), "9");
}

#[test]
fn dispatch_literal_then_bind_fallback() {
    use athena_engine::reasoning::trs::TermPattern;
    use athena_ir::{Atom, TermNode};
    let mut c = C::new();
    let f_op = c.s.operators.intern("f");
    let f_sym = c.s.arena.symbols_mut().intern("f");
    let one = i(1, &mut c);
    let ten = i(10, &mut c);
    c.s.defs.register_rule(
        f_sym,
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
    c.s.defs.register_rule(
        f_sym,
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
    let result_id = execute_ir_request(&mut c.s, request).expect("ir");
    let term = c.s.results.get(result_id).and_then(|r| r.symbolic_term).expect("term");
    assert_eq!(term_debug(&c.s, term), "List[1, 2, 3]");
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
fn machine_trig_at_real_points() {
    let mut c = C::new();
    let zero = {
        let span = athena_ir::TermNode::default_span();
        c.s.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::Number(athena_numeric::NumericValue::machine(0.0))), span)
    };
    let e = unary(UnaryFunction::Sin, vec![zero], &mut c);
    assert_eq!(t(e, &mut c), "0");
    let e = unary(UnaryFunction::Cos, vec![symbol("Pi", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "-1");
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
fn unsupported_import_is_not_silent_value() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    let e = ext("Import", vec![str_("x.csv", &mut c)], &mut c);
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::UnsupportedOperation);
}

#[test]
fn if_false_and_null_and_non_boolean() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    assert_eq!(t(ext("Branch", vec![symbol("False", &mut c), i(7, &mut c), i(8, &mut c)], &mut c), &mut c), "8");
    assert_eq!(t(ext("Branch", vec![i(0, &mut c), i(7, &mut c)], &mut c), &mut c), "Null");
    let e = ext("Branch", vec![symbol("x", &mut c), i(1, &mut c), i(2, &mut c)], &mut c);
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::NonBooleanCondition);
}

#[test]
fn branch_true_skips_else_import() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    let e = ext("Branch", vec![symbol("True", &mut c), i(7, &mut c), ext("Import", vec![str_("x.csv", &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "7");
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Exact);
    assert!(!o.diagnostics.iter().any(|d| d.code == DiagnosticCode::UnsupportedOperation));
}
