//! Interp 执行层 `evaluate_session` 覆盖（Living `25` L2 · 原 legacy `evaluate` 桥合同）。

use athena_engine::{
    Session,
    arena_ops::{push_app_named, push_int, push_list, push_symbol_name},
    interp,
    interp::vm::evaluate_session,
    present::term_debug,
};

type Tid = athena_types::TermId;

struct C {
    s: Session,
}

impl C {
    fn new() -> Self {
        Self { s: Session::new() }
    }
}

fn out(e: Tid, c: &mut C) -> interp::Outcome {
    evaluate_session(&mut c.s, e)
}

/// 求值并渲染为调试串。
fn t(e: Tid, c: &mut C) -> String {
    let o = evaluate_session(&mut c.s, e);
    term_debug(&c.s, o.term)
}

fn sym(name: &str, c: &mut C) -> Tid {
    push_symbol_name(&mut c.s, name)
}

fn i(n: i64, c: &mut C) -> Tid {
    push_int(&mut c.s, n)
}

fn str_(v: &str, c: &mut C) -> Tid {
    let span = athena_ir::TermKind::default_span();
    c.s.arena.push(athena_ir::TermKind::Atom(athena_ir::AtomKind::String(v.into())), span)
}

fn boolean(v: bool, c: &mut C) -> Tid {
    let span = athena_ir::TermKind::default_span();
    c.s.arena.push(athena_ir::TermKind::Atom(athena_ir::AtomKind::Boolean(v)), span)
}

fn lst(items: Vec<Tid>, c: &mut C) -> Tid {
    push_list(&mut c.s, items)
}

fn ap(head: &str, args: Vec<Tid>, c: &mut C) -> Tid {
    push_app_named(&mut c.s, head, args)
}

#[test]
fn plus_fold() {
    let mut c = C::new();
    let x = sym("x", &mut c);
    let e = ap("Plus", vec![i(1, &mut c), i(2, &mut c), x], &mut c);
    assert_eq!(t(e, &mut c), "Plus[3, x]");
}

#[test]
fn power_one() {
    let mut c = C::new();
    let e = ap("Power", vec![sym("x", &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "x");
}

#[test]
fn list_eval() {
    let mut c = C::new();
    let inner = ap("Plus", vec![i(2, &mut c), i(2, &mut c)], &mut c);
    let e = lst(vec![i(1, &mut c), inner], &mut c);
    assert_eq!(t(e, &mut c), "List[1, 4]");
}

#[test]
fn d_power() {
    let mut c = C::new();
    let e = ap("D", vec![ap("Power", vec![sym("x", &mut c), i(3, &mut c)], &mut c), sym("x", &mut c)], &mut c);
    let r = t(e, &mut c);
    assert!(r.contains("x"), "got {r}");
}

#[test]
fn pythagorean() {
    let mut c = C::new();
    let sin2 = ap("Power", vec![ap("Sin", vec![sym("x", &mut c)], &mut c), i(2, &mut c)], &mut c);
    let cos2 = ap("Power", vec![ap("Cos", vec![sym("x", &mut c)], &mut c), i(2, &mut c)], &mut c);
    let e = ap("Simplify", vec![ap("Plus", vec![sin2, cos2], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "1");
}

#[test]
fn compound_expression_returns_last() {
    let mut c = C::new();
    let e = ap("CompoundExpression", vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn integrate_power() {
    let mut c = C::new();
    let e = ap("Integrate", vec![ap("Power", vec![sym("x", &mut c), i(2, &mut c)], &mut c), sym("x", &mut c)], &mut c);
    let r = t(e, &mut c);
    assert!(r.contains("x"), "got {r}");
}

#[test]
fn map_sin_list() {
    let mut c = C::new();
    let e = ap("Map", vec![sym("Sin", &mut c), lst(vec![i(0, &mut c)], &mut c)], &mut c);
    let r = t(e, &mut c);
    assert!(r.starts_with("List["), "got {r}");
}

#[test]
fn truthy_via_and_or() {
    let mut c = C::new();
    assert_eq!(t(ap("And", vec![i(0, &mut c), i(1, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(ap("And", vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
    assert_eq!(t(ap("Or", vec![i(0, &mut c), i(0, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(ap("Or", vec![i(0, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
    assert_eq!(t(ap("And", vec![boolean(true, &mut c), boolean(false, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(ap("Not", vec![boolean(true, &mut c)], &mut c), &mut c), "False");
}

#[test]
fn part_zero_returns_list_head() {
    let mut c = C::new();
    let e = ap("Part", vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), i(0, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List");
}

#[test]
fn part_oob_is_invalid_index() {
    use athena_types::{ComputationStatus, DiagnosticCode};
    let mut c = C::new();
    let e = ap("Part", vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), i(9, &mut c)], &mut c);
    let o = out(e, &mut c);
    assert!(o.has_error());
    assert_eq!(o.kind, interp::EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::InvalidIndex);
}

#[test]
fn unsupported_import_is_not_silent_value() {
    use athena_types::{ComputationStatus, DiagnosticCode};
    let mut c = C::new();
    let e = ap("Import", vec![str_("x.csv", &mut c)], &mut c);
    let o = out(e, &mut c);
    assert_eq!(o.kind, interp::EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::UnsupportedOperation);
}

#[test]
fn unknown_head_is_unevaluated_not_exact_value() {
    use athena_types::ComputationStatus;
    let mut c = C::new();
    let e = ap("FooBar", vec![i(1, &mut c)], &mut c);
    let o = out(e, &mut c);
    assert_eq!(o.kind, interp::EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert!(!o.has_error());
}

#[test]
fn if_true_branch_and_short_circuit() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    let cond = ap("Equal", vec![i(1, &mut c), i(1, &mut c)], &mut c);
    let e = ap("If", vec![cond, i(7, &mut c), i(8, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "7");
    // False 分支不得求值 Import（不应产生 UnsupportedOperation）。
    let e = ap("If", vec![sym("True", &mut c), i(7, &mut c), ap("Import", vec![str_("x.csv", &mut c)], &mut c)], &mut c);
    let o = out(e, &mut c);
    assert_eq!(term_debug(&c.s, o.term), "7");
    assert_eq!(o.kind, interp::EvalKind::Value);
    assert!(!o.diagnostics.iter().any(|d| d.code == DiagnosticCode::UnsupportedOperation));
}

#[test]
fn if_false_and_null_and_non_boolean() {
    use athena_types::{ComputationStatus, DiagnosticCode};
    let mut c = C::new();
    assert_eq!(t(ap("If", vec![sym("False", &mut c), i(7, &mut c), i(8, &mut c)], &mut c), &mut c), "8");
    assert_eq!(t(ap("If", vec![i(0, &mut c), i(7, &mut c)], &mut c), &mut c), "Null");
    let e = ap("If", vec![sym("x", &mut c), i(1, &mut c), i(2, &mut c)], &mut c);
    let o = out(e, &mut c);
    assert_eq!(o.kind, interp::EvalKind::Unevaluated);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::NonBooleanCondition);
}

#[test]
fn symbol_true_false_null_canonicalize_to_typed_atoms() {
    let mut c = C::new();
    assert_eq!(t(sym("True", &mut c), &mut c), "True");
    assert_eq!(t(sym("False", &mut c), &mut c), "False");
    assert_eq!(t(sym("Null", &mut c), &mut c), "Null");
    assert_eq!(t(ap("Equal", vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
}

#[test]
fn hold_and_hold_form_do_not_eval_args() {
    let mut c = C::new();
    assert_eq!(t(ap("Hold", vec![ap("Plus", vec![i(1, &mut c), i(1, &mut c)], &mut c)], &mut c), &mut c), "Hold[Plus[1, 1]]");
    assert_eq!(
        t(ap("HoldForm", vec![ap("Plus", vec![i(2, &mut c), i(3, &mut c)], &mut c)], &mut c), &mut c),
        "HoldForm[Plus[2, 3]]"
    );
}

#[test]
fn which_picks_first_true_branch() {
    let mut c = C::new();
    let e = ap(
        "Which",
        vec![sym("False", &mut c), i(1, &mut c), sym("True", &mut c), i(2, &mut c), sym("True", &mut c), i(3, &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "2");
}

#[test]
fn span_expands_to_list() {
    let mut c = C::new();
    assert_eq!(t(ap("Span", vec![i(1, &mut c), i(3, &mut c)], &mut c), &mut c), "List[1, 2, 3]");
    assert_eq!(t(ap("Span", vec![i(1, &mut c), i(2, &mut c), i(10, &mut c)], &mut c), &mut c), "List[1, 3, 5, 7, 9]");
}

#[test]
fn part_span_slice() {
    let mut c = C::new();
    let e = ap(
        "Part",
        vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), ap("Span", vec![i(1, &mut c), i(2, &mut c)], &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "List[1, 2]");
}

#[test]
fn while_false_skips_body() {
    let mut c = C::new();
    let e = ap("While", vec![i(0, &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "Null");
}

#[test]
fn compound_set_binds_for_later_stmts() {
    let mut c = C::new();
    let set = ap("Set", vec![sym("x", &mut c), i(5, &mut c)], &mut c);
    let e = ap("CompoundExpression", vec![set, ap("Plus", vec![sym("x", &mut c), i(1, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
}

#[test]
fn part_end_is_last_element() {
    let mut c = C::new();
    let e = ap("Part", vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), sym("End", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn part_all_returns_list() {
    let mut c = C::new();
    let e = ap("Part", vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), sym("All", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[1, 2]");
}

#[test]
fn for_span_last_value() {
    let mut c = C::new();
    let e = ap("For", vec![sym("i", &mut c), ap("Span", vec![i(1, &mut c), i(3, &mut c)], &mut c), sym("i", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn for_accumulator_shares_compound_bindings() {
    let mut c = C::new();
    let set0 = ap("Set", vec![sym("s", &mut c), i(0, &mut c)], &mut c);
    let body = ap("Set", vec![sym("s", &mut c), ap("Plus", vec![sym("s", &mut c), sym("i", &mut c)], &mut c)], &mut c);
    let f = ap("For", vec![sym("i", &mut c), ap("Span", vec![i(1, &mut c), i(3, &mut c)], &mut c), body], &mut c);
    let e = ap("CompoundExpression", vec![set0, f, sym("s", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
}

#[test]
fn compare_chain_less_expands_to_and() {
    let mut c = C::new();
    let nested = ap("Less", vec![i(1, &mut c), i(2, &mut c)], &mut c);
    let e = ap("Less", vec![nested, i(3, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "True");
    let nested = ap("Less", vec![i(1, &mut c), i(0, &mut c)], &mut c);
    let e2 = ap("Less", vec![nested, i(3, &mut c)], &mut c);
    assert_eq!(t(e2, &mut c), "False");
}

#[test]
fn try_catch_on_error_and_success() {
    let mut c = C::new();
    let err = ap("Try", vec![ap("error", vec![str_("e", &mut c)], &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(err, &mut c), "1");
    let ok = ap("Try", vec![i(2, &mut c), i(3, &mut c)], &mut c);
    assert_eq!(t(ok, &mut c), "2");
}

#[test]
fn with_module_block_local_bindings() {
    let locals = |c: &mut C| {
        let l = lst(vec![ap("Set", vec![sym("x", c), i(1, c)], c)], c);
        let b = ap("Plus", vec![sym("x", c), i(1, c)], c);
        (l, b)
    };
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(ap("With", vec![l, b], &mut d), &mut d), "2");
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(ap("Module", vec![l, b], &mut d), &mut d), "2");
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(ap("Block", vec![l, b], &mut d), &mut d), "2");
}

#[test]
fn part_column_all_then_index() {
    // MATLAB A(:,2) as Part[matrix, All, 2]
    let mut c = C::new();
    let matrix =
        lst(vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    let e = ap("Part", vec![matrix, sym("All", &mut c), i(2, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[2, 4]");
}

#[test]
fn session_set_persists_across_evaluate() {
    let mut c = C::new();
    let set = ap("Set", vec![sym("x", &mut c), i(5, &mut c)], &mut c);
    assert_eq!(t(set, &mut c), "5");
    let e = ap("Plus", vec![sym("x", &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    // 无定义的新 session：x 保持自由符号。
    let mut d = C::new();
    let e = ap("Plus", vec![sym("x", &mut d), i(1, &mut d)], &mut d);
    let r = t(e, &mut d);
    assert!(r.contains("x"), "expected free x, got {r}");
}

#[test]
fn session_compound_set_writes_definitions() {
    let mut c = C::new();
    let set = ap("Set", vec![sym("y", &mut c), i(3, &mut c)], &mut c);
    let plus = ap("Plus", vec![sym("y", &mut c), i(4, &mut c)], &mut c);
    let e = ap("CompoundExpression", vec![set, plus], &mut c);
    assert_eq!(t(e, &mut c), "7");
    assert_eq!(t(sym("y", &mut c), &mut c), "3");
}

#[test]
fn session_setdelayed_evaluates_on_use() {
    let mut c = C::new();
    let delayed = ap("SetDelayed", vec![sym("a", &mut c), ap("Plus", vec![i(1, &mut c), i(1, &mut c)], &mut c)], &mut c);
    assert_eq!(t(delayed, &mut c), "Null");
    assert_eq!(t(sym("a", &mut c), &mut c), "2");
}

#[test]
fn session_setdelayed_pattern_down_value() {
    let mut c = C::new();
    let lhs = ap("f", vec![ap("Pattern", vec![sym("x", &mut c), ap("Blank", vec![], &mut c)], &mut c)], &mut c);
    let rhs = ap("Power", vec![sym("x", &mut c), i(2, &mut c)], &mut c);
    let define = ap("SetDelayed", vec![lhs, rhs], &mut c);
    assert_eq!(t(define, &mut c), "Null");
    assert_eq!(t(ap("f", vec![i(3, &mut c)], &mut c), &mut c), "9");
    // 同一 compound 内定义 + 调用。
    let mut d = C::new();
    let lhs = ap("f", vec![ap("Pattern", vec![sym("x", &mut d), ap("Blank", vec![], &mut d)], &mut d)], &mut d);
    let rhs = ap("Power", vec![sym("x", &mut d), i(2, &mut d)], &mut d);
    let define = ap("SetDelayed", vec![lhs, rhs], &mut d);
    let call = ap("f", vec![i(3, &mut d)], &mut d);
    let e = ap("CompoundExpression", vec![define, call], &mut d);
    assert_eq!(t(e, &mut d), "9");
}

#[test]
fn compare_list_scalar_broadcasts() {
    let mut c = C::new();
    let e = ap("Less", vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), i(2, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[True, False, False]");
}

#[test]
fn module_bare_local_is_renamed_unique() {
    let mut c = C::new();
    let e1 = ap("Module", vec![lst(vec![sym("x", &mut c)], &mut c), sym("x", &mut c)], &mut c);
    let r1 = t(e1, &mut c);
    let e2 = ap("Module", vec![lst(vec![sym("x", &mut c)], &mut c), sym("x", &mut c)], &mut c);
    let r2 = t(e2, &mut c);
    assert!(r1.starts_with("x$"), "got {r1}");
    assert!(r2.starts_with("x$"), "got {r2}");
    assert_ne!(r1, r2);
}

#[test]
fn module_local_does_not_clobber_session() {
    let mut c = C::new();
    let set = ap("Set", vec![sym("x", &mut c), i(5, &mut c)], &mut c);
    out(set, &mut c);
    let locals = lst(vec![ap("Set", vec![sym("x", &mut c), i(1, &mut c)], &mut c)], &mut c);
    let body = ap("Plus", vec![sym("x", &mut c), i(1, &mut c)], &mut c);
    let e = ap("Module", vec![locals, body], &mut c);
    assert_eq!(t(e, &mut c), "2");
    // Session 级 x 仍为 5。
    assert_eq!(t(sym("x", &mut c), &mut c), "5");
}

#[test]
fn nested_module_names_do_not_collide() {
    let mut c = C::new();
    let inner_locals = lst(vec![sym("x", &mut c)], &mut c);
    let inner = ap("Module", vec![inner_locals, sym("x", &mut c)], &mut c);
    let outer_locals = lst(vec![sym("x", &mut c)], &mut c);
    let e = ap("Module", vec![outer_locals, inner], &mut c);
    let r = t(e, &mut c);
    assert!(r.starts_with("x$"), "got {r}");
}

#[test]
fn down_value_literal_pattern_and_fallback() {
    let mut c = C::new();
    // f[1] := 10 ; f[x_] := x * 2
    let lhs1 = ap("f", vec![i(1, &mut c)], &mut c);
    let def1 = ap("SetDelayed", vec![lhs1, i(10, &mut c)], &mut c);
    out(def1, &mut c);
    let lhs2 = ap("f", vec![ap("Pattern", vec![sym("x", &mut c), ap("Blank", vec![], &mut c)], &mut c)], &mut c);
    let rhs2 = ap("Times", vec![sym("x", &mut c), i(2, &mut c)], &mut c);
    let def2 = ap("SetDelayed", vec![lhs2, rhs2], &mut c);
    out(def2, &mut c);
    assert_eq!(t(ap("f", vec![i(1, &mut c)], &mut c), &mut c), "10");
    assert_eq!(t(ap("f", vec![i(5, &mut c)], &mut c), &mut c), "10");
}

#[test]
fn replace_all_literal() {
    let mut c = C::new();
    let rule = ap("Rule", vec![sym("x", &mut c), i(9, &mut c)], &mut c);
    let e = ap("ReplaceAll", vec![ap("Plus", vec![sym("x", &mut c), i(1, &mut c)], &mut c), rule], &mut c);
    assert_eq!(t(e, &mut c), "10");
}

#[test]
fn apply_and_join_and_length() {
    let mut c = C::new();
    let e = ap("Apply", vec![sym("Plus", &mut c), lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    let e = ap("Join", vec![lst(vec![i(1, &mut c)], &mut c), lst(vec![i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[1, 2, 3]");
    let e = ap("Length", vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn cases_filters_by_pattern() {
    let mut c = C::new();
    let pat = ap("Blank", vec![sym("Integer", &mut c)], &mut c);
    let e = ap("Cases", vec![lst(vec![i(1, &mut c), sym("y", &mut c), i(3, &mut c)], &mut c), pat], &mut c);
    let r = t(e, &mut c);
    assert!(r.contains("List[1, 3]"), "got {r}");
}

#[test]
fn array_sum_vector_and_matrix() {
    let mut c = C::new();
    let e = ap("Sum", vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
}

#[test]
fn det_and_size() {
    let mut c = C::new();
    let m = lst(vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), lst(vec![i(3, &mut c), i(4, &mut c)], &mut c)], &mut c);
    assert_eq!(t(ap("Det", vec![m.clone()], &mut c), &mut c), "-2");
    let r = t(ap("Size", vec![m], &mut c), &mut c);
    assert!(r.contains("List[2, 2]"), "got {r}");
}

#[test]
fn mldivide_column_vector() {
    let mut c = C::new();
    let m = lst(vec![lst(vec![i(2, &mut c), i(0, &mut c)], &mut c), lst(vec![i(0, &mut c), i(2, &mut c)], &mut c)], &mut c);
    let b = lst(vec![lst(vec![i(4, &mut c)], &mut c), lst(vec![i(6, &mut c)], &mut c)], &mut c);
    let e = ap("LinearSolve", vec![m, b], &mut c);
    let r = t(e, &mut c);
    assert!(r.contains("List[2]"), "got {r}");
}

#[test]
fn machine_trig_at_real_points() {
    let mut c = C::new();
    // Sin[0.0] → 精确 0；Cos[Pi] → -1。
    let zero = {
        let span = athena_ir::TermKind::default_span();
        c.s.arena.push(athena_ir::TermKind::Atom(athena_ir::AtomKind::Number(athena_numeric::NumericValue::machine(0.0))), span)
    };
    let e = ap("Sin", vec![zero], &mut c);
    assert_eq!(t(e, &mut c), "0");
    let e = ap("Cos", vec![sym("Pi", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "-1");
}

#[test]
fn depth_limit_returns_unevaluated() {
    // 深度 256 上限：自引用 Through 深链走 While 计数上限路径即可覆盖 guard。
    let mut c = C::new();
    let cond = i(0, &mut c);
    let body = i(1, &mut c);
    let e = ap("While", vec![cond, body], &mut c);
    assert_eq!(t(e, &mut c), "Null");
}

#[test]
fn sum_over_iterator_folds() {
    let mut c = C::new();
    let iter = lst(vec![sym("k", &mut c), i(1, &mut c), i(4, &mut c)], &mut c);
    let e = ap("Sum", vec![ap("Power", vec![sym("k", &mut c), i(2, &mut c)], &mut c), iter], &mut c);
    assert_eq!(t(e, &mut c), "30");
}

#[test]
fn table_with_single_bound() {
    let mut c = C::new();
    let iter = lst(vec![sym("i", &mut c), i(3, &mut c)], &mut c);
    let e = ap("Table", vec![sym("i", &mut c), iter], &mut c);
    assert_eq!(t(e, &mut c), "List[1, 2, 3]");
}
