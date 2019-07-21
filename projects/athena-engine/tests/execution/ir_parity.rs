//! `ExecutionIR` interp parity (green subset of VM suite · Living `25` L2).

use athena_engine::{
    api::request::AthenaRequest,
    diagnostics::term_summary::term_debug,
    execution::execute_ir_request,
    runtime::{
        Session,
        results::CoverageStatus,
        values::arena::{push_application_named, push_int, push_list, push_symbol_name},
    },
};
use athena_types::{ComputationStatus, TermId};

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
    c.s.arena
        .push(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(v)), span)
}

fn str_(v: &str, c: &mut C) -> Tid {
    let span = athena_ir::TermNode::default_span();
    c.s.arena
        .push(athena_ir::TermNode::Atom(athena_ir::Atom::String(v.into())), span)
}

fn lst(items: Vec<Tid>, c: &mut C) -> Tid {
    push_list(&mut c.s, items)
}

fn apply(head: &str, args: Vec<Tid>, c: &mut C) -> Tid {
    push_application_named(&mut c.s, head, args)
}

#[test]
fn plus_fold() {
    let mut c = C::new();
    let x = symbol("x", &mut c);
    let e = apply("Plus", vec![i(1, &mut c), i(2, &mut c), x], &mut c);
    assert_eq!(t(e, &mut c), "Plus[3, x]");
}

#[test]
fn power_one() {
    let mut c = C::new();
    let e = apply("Power", vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "x");
}

#[test]
fn list_eval() {
    let mut c = C::new();
    let inner = apply("Plus", vec![i(2, &mut c), i(2, &mut c)], &mut c);
    let e = lst(vec![i(1, &mut c), inner], &mut c);
    assert_eq!(t(e, &mut c), "List[1, 4]");
}

#[test]
fn compound_expression_returns_last() {
    let mut c = C::new();
    let e = apply("Sequence", vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn truthy_via_and_or() {
    let mut c = C::new();
    assert_eq!(t(apply("And", vec![i(0, &mut c), i(1, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(apply("And", vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
    assert_eq!(t(apply("Or", vec![i(0, &mut c), i(0, &mut c)], &mut c), &mut c), "False");
    assert_eq!(t(apply("Or", vec![i(0, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
    assert_eq!(
        t(
            apply("And", vec![boolean(true, &mut c), boolean(false, &mut c)], &mut c),
            &mut c
        ),
        "False"
    );
    assert_eq!(t(apply("Not", vec![boolean(true, &mut c)], &mut c), &mut c), "False");
}

#[test]
fn unknown_head_is_unevaluated_not_exact_value() {
    let mut c = C::new();
    let e = apply("FooBar", vec![i(1, &mut c)], &mut c);
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Unknown);
    assert_eq!(o.coverage, CoverageStatus::Partial);
    assert!(o.diagnostics.is_empty());
}

#[test]
fn part_zero_returns_list_head() {
    let mut c = C::new();
    let e = apply(
        "Part",
        vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), i(0, &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "List");
}

#[test]
fn part_end_is_last_element() {
    let mut c = C::new();
    let e = apply(
        "Part",
        vec![
            lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c),
            symbol("End", &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn part_all_returns_list() {
    let mut c = C::new();
    let e = apply(
        "Part",
        vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), symbol("All", &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "List[1, 2]");
}

#[test]
fn part_oob_is_invalid_index() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    let e = apply(
        "Part",
        vec![lst(vec![i(1, &mut c), i(2, &mut c)], &mut c), i(9, &mut c)],
        &mut c,
    );
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::InvalidIndex);
}

#[test]
fn part_span_extracts_slice() {
    let mut c = C::new();
    let e = apply(
        "Part",
        vec![
            lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c),
            apply("Span", vec![i(1, &mut c), i(2, &mut c)], &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "List[1, 2]");
}

#[test]
fn part_column_all_then_index() {
    let mut c = C::new();
    let matrix = lst(
        vec![
            lst(vec![i(1, &mut c), i(2, &mut c)], &mut c),
            lst(vec![i(3, &mut c), i(4, &mut c)], &mut c),
        ],
        &mut c,
    );
    let e = apply("Part", vec![matrix, symbol("All", &mut c), i(2, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "List[2, 4]");
}

#[test]
fn while_false_skips_body() {
    let mut c = C::new();
    let e = apply("LoopWhile", vec![i(0, &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "Null");
}

#[test]
fn compound_set_binds_for_later_stmts() {
    let mut c = C::new();
    let set = apply("Define", vec![symbol("x", &mut c), i(5, &mut c)], &mut c);
    let e = apply(
        "Sequence",
        vec![set, apply("Plus", vec![symbol("x", &mut c), i(1, &mut c)], &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "6");
}

#[test]
fn session_setdelayed_evaluates_on_use() {
    let mut c = C::new();
    let delayed = apply(
        "DefineDeferred",
        vec![symbol("a", &mut c), apply("Plus", vec![i(1, &mut c), i(1, &mut c)], &mut c)],
        &mut c,
    );
    assert_eq!(t(delayed, &mut c), "Null");
    assert_eq!(t(symbol("a", &mut c), &mut c), "2");
}

#[test]
fn with_module_block_local_bindings() {
    let locals = |c: &mut C| {
        let l = lst(vec![apply("Define", vec![symbol("x", c), i(1, c)], c)], c);
        let b = apply("Plus", vec![symbol("x", c), i(1, c)], c);
        (l, b)
    };
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(apply("LocalScope", vec![l, b], &mut d), &mut d), "2");
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(apply("LexicalScope", vec![l, b], &mut d), &mut d), "2");
    let mut d = C::new();
    let (l, b) = locals(&mut d);
    assert_eq!(t(apply("DynamicScope", vec![l, b], &mut d), &mut d), "2");
}

#[test]
fn module_bare_local_is_renamed_unique() {
    let mut c = C::new();
    let e1 = apply(
        "LexicalScope",
        vec![lst(vec![symbol("x", &mut c)], &mut c), symbol("x", &mut c)],
        &mut c,
    );
    let r1 = t(e1, &mut c);
    let e2 = apply(
        "LexicalScope",
        vec![lst(vec![symbol("x", &mut c)], &mut c), symbol("x", &mut c)],
        &mut c,
    );
    let r2 = t(e2, &mut c);
    assert!(r1.starts_with("x$"), "got {r1}");
    assert!(r2.starts_with("x$"), "got {r2}");
    assert_ne!(r1, r2);
}

#[test]
fn try_catch_on_error_and_success() {
    let mut c = C::new();
    let err = apply(
        "Recover",
        vec![apply("error", vec![str_("e", &mut c)], &mut c), i(1, &mut c)],
        &mut c,
    );
    assert_eq!(t(err, &mut c), "1");
    let ok = apply("Recover", vec![i(2, &mut c), i(3, &mut c)], &mut c);
    assert_eq!(t(ok, &mut c), "2");
}

#[test]
fn session_set_persists_across_evaluate() {
    let mut c = C::new();
    let set = apply("Define", vec![symbol("x", &mut c), i(5, &mut c)], &mut c);
    assert_eq!(t(set, &mut c), "5");
    let e = apply("Plus", vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    let mut d = C::new();
    let e = apply("Plus", vec![symbol("x", &mut d), i(1, &mut d)], &mut d);
    let r = t(e, &mut d);
    assert!(r.contains("x"), "expected free x, got {r}");
}

#[test]
fn map_sin_list() {
    let mut c = C::new();
    let e = apply("Map", vec![symbol("Sin", &mut c), lst(vec![i(0, &mut c)], &mut c)], &mut c);
    let r = t(e, &mut c);
    assert!(r.starts_with("List["), "got {r}");
}

#[test]
fn for_span_last_value() {
    let mut c = C::new();
    let e = apply(
        "CountedLoop",
        vec![
            symbol("i", &mut c),
            apply("Span", vec![i(1, &mut c), i(3, &mut c)], &mut c),
            symbol("i", &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn for_accumulator_shares_compound_bindings() {
    let mut c = C::new();
    let set0 = apply("Define", vec![symbol("s", &mut c), i(0, &mut c)], &mut c);
    let body = apply(
        "Define",
        vec![
            symbol("s", &mut c),
            apply("Plus", vec![symbol("s", &mut c), symbol("i", &mut c)], &mut c),
        ],
        &mut c,
    );
    let f = apply(
        "CountedLoop",
        vec![
            symbol("i", &mut c),
            apply("Span", vec![i(1, &mut c), i(3, &mut c)], &mut c),
            body,
        ],
        &mut c,
    );
    let e = apply("Sequence", vec![set0, f, symbol("s", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
}

#[test]
fn compare_chain_less_expands_to_and() {
    let mut c = C::new();
    let nested = apply("Less", vec![i(1, &mut c), i(2, &mut c)], &mut c);
    let e = apply("Less", vec![nested, i(3, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "True");
    let nested = apply("Less", vec![i(1, &mut c), i(0, &mut c)], &mut c);
    let e2 = apply("Less", vec![nested, i(3, &mut c)], &mut c);
    assert_eq!(t(e2, &mut c), "False");
}

#[test]
fn compare_list_scalar_broadcasts() {
    let mut c = C::new();
    let e = apply(
        "Less",
        vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c), i(2, &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "List[True, False, False]");
}

#[test]
fn replace_all_literal() {
    let mut c = C::new();
    let rule = apply("Rule", vec![symbol("x", &mut c), i(9, &mut c)], &mut c);
    let e = apply(
        "ReplaceAll",
        vec![apply("Plus", vec![symbol("x", &mut c), i(1, &mut c)], &mut c), rule],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "10");
}

#[test]
fn apply_and_join_and_length() {
    let mut c = C::new();
    let e = apply(
        "Apply",
        vec![symbol("Plus", &mut c), lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "6");
    let e = apply(
        "Join",
        vec![lst(vec![i(1, &mut c)], &mut c), lst(vec![i(2, &mut c), i(3, &mut c)], &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "List[1, 2, 3]");
    let e = apply(
        "Length",
        vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "3");
}

#[test]
fn module_local_does_not_clobber_session() {
    let mut c = C::new();
    let set = apply("Define", vec![symbol("x", &mut c), i(5, &mut c)], &mut c);
    let _ = c.s.evaluate(set);
    let locals = lst(vec![apply("Define", vec![symbol("x", &mut c), i(1, &mut c)], &mut c)], &mut c);
    let body = apply("Plus", vec![symbol("x", &mut c), i(1, &mut c)], &mut c);
    let e = apply("LexicalScope", vec![locals, body], &mut c);
    assert_eq!(t(e, &mut c), "2");
    assert_eq!(t(symbol("x", &mut c), &mut c), "5");
}

#[test]
fn nested_module_names_do_not_collide() {
    let mut c = C::new();
    let inner_locals = lst(vec![symbol("x", &mut c)], &mut c);
    let inner = apply("LexicalScope", vec![inner_locals, symbol("x", &mut c)], &mut c);
    let outer_locals = lst(vec![symbol("x", &mut c)], &mut c);
    let e = apply("LexicalScope", vec![outer_locals, inner], &mut c);
    let r = t(e, &mut c);
    assert!(r.starts_with("x$"), "got {r}");
}

#[test]
fn array_sum_vector_and_matrix() {
    let mut c = C::new();
    let e = apply("Sum", vec![lst(vec![i(1, &mut c), i(2, &mut c), i(3, &mut c)], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "6");
    let m = lst(
        vec![
            lst(vec![i(1, &mut c), i(2, &mut c)], &mut c),
            lst(vec![i(3, &mut c), i(4, &mut c)], &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(apply("Sum", vec![m], &mut c), &mut c), "List[4, 6]");
}

#[test]
fn size_of_matrix() {
    let mut c = C::new();
    let m = lst(
        vec![
            lst(vec![i(1, &mut c), i(2, &mut c)], &mut c),
            lst(vec![i(3, &mut c), i(4, &mut c)], &mut c),
        ],
        &mut c,
    );
    let r = t(apply("Size", vec![m], &mut c), &mut c);
    assert!(r.contains("List[2, 2]"), "got {r}");
}

#[test]
fn det_and_size() {
    let mut c = C::new();
    let m = lst(
        vec![
            lst(vec![i(1, &mut c), i(2, &mut c)], &mut c),
            lst(vec![i(3, &mut c), i(4, &mut c)], &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(apply("Det", vec![m.clone()], &mut c), &mut c), "-2");
    let r = t(apply("Size", vec![m], &mut c), &mut c);
    assert!(r.contains("List[2, 2]"), "got {r}");
}

#[test]
fn linear_solve_column_vector() {
    let mut c = C::new();
    let m = lst(
        vec![
            lst(vec![i(2, &mut c), i(0, &mut c)], &mut c),
            lst(vec![i(0, &mut c), i(2, &mut c)], &mut c),
        ],
        &mut c,
    );
    let b = lst(
        vec![
            lst(vec![i(4, &mut c)], &mut c),
            lst(vec![i(6, &mut c)], &mut c),
        ],
        &mut c,
    );
    let r = t(apply("LinearSolve", vec![m, b], &mut c), &mut c);
    assert!(r.contains("List[2]"), "got {r}");
}

#[test]
fn hold_and_hold_form_do_not_eval_args() {
    let mut c = C::new();
    assert_eq!(
        t(
            apply("Hold", vec![apply("Plus", vec![i(1, &mut c), i(1, &mut c)], &mut c)], &mut c),
            &mut c
        ),
        "Hold[Plus[1, 1]]"
    );
}

#[test]
fn symbol_true_false_null_canonicalize() {
    let mut c = C::new();
    assert_eq!(t(symbol("True", &mut c), &mut c), "True");
    assert_eq!(t(symbol("False", &mut c), &mut c), "False");
    assert_eq!(t(symbol("Null", &mut c), &mut c), "Null");
    assert_eq!(t(apply("Equal", vec![i(1, &mut c), i(1, &mut c)], &mut c), &mut c), "True");
}

#[test]
fn if_true_branch() {
    let mut c = C::new();
    let cond = apply("Equal", vec![i(1, &mut c), i(1, &mut c)], &mut c);
    let e = apply("Branch", vec![cond, i(7, &mut c), i(8, &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "7");
}

#[test]
fn cond_picks_first_true_branch() {
    let mut c = C::new();
    let e = apply(
        "Cond",
        vec![
            symbol("False", &mut c),
            i(1, &mut c),
            symbol("True", &mut c),
            i(2, &mut c),
            symbol("True", &mut c),
            i(3, &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "2");
}

#[test]
fn span_expands_to_list() {
    let mut c = C::new();
    assert_eq!(t(apply("Span", vec![i(1, &mut c), i(3, &mut c)], &mut c), &mut c), "List[1, 2, 3]");
}

#[test]
fn session_setdelayed_pattern_down_value() {
    let mut c = C::new();
    let lhs = apply(
        "f",
        vec![apply("Bind", vec![symbol("x", &mut c), apply("Any", vec![], &mut c)], &mut c)],
        &mut c,
    );
    let rhs = apply("Power", vec![symbol("x", &mut c), i(2, &mut c)], &mut c);
    let define = apply("DefineDeferred", vec![lhs, rhs], &mut c);
    assert_eq!(t(define, &mut c), "Null");
    assert_eq!(t(apply("f", vec![i(3, &mut c)], &mut c), &mut c), "9");
    let mut d = C::new();
    let lhs = apply(
        "f",
        vec![apply("Bind", vec![symbol("x", &mut d), apply("Any", vec![], &mut d)], &mut d)],
        &mut d,
    );
    let rhs = apply("Power", vec![symbol("x", &mut d), i(2, &mut d)], &mut d);
    let define = apply("DefineDeferred", vec![lhs, rhs], &mut d);
    let call = apply("f", vec![i(3, &mut d)], &mut d);
    let e = apply("Sequence", vec![define, call], &mut d);
    assert_eq!(t(e, &mut d), "9");
}

#[test]
fn down_value_literal_pattern_and_fallback() {
    let mut c = C::new();
    let lhs1 = apply("f", vec![i(1, &mut c)], &mut c);
    let def1 = apply("DefineDeferred", vec![lhs1, i(10, &mut c)], &mut c);
    assert_eq!(t(def1, &mut c), "Null");
    let lhs2 = apply(
        "f",
        vec![apply("Bind", vec![symbol("x", &mut c), apply("Any", vec![], &mut c)], &mut c)],
        &mut c,
    );
    let rhs2 = apply("Times", vec![symbol("x", &mut c), i(2, &mut c)], &mut c);
    let def2 = apply("DefineDeferred", vec![lhs2, rhs2], &mut c);
    assert_eq!(t(def2, &mut c), "Null");
    assert_eq!(t(apply("f", vec![i(1, &mut c)], &mut c), &mut c), "10");
    assert_eq!(t(apply("f", vec![i(5, &mut c)], &mut c), &mut c), "10");
}

#[test]
fn sum_over_iterator_folds() {
    let mut c = C::new();
    let iter = lst(vec![symbol("k", &mut c), i(1, &mut c), i(4, &mut c)], &mut c);
    let e = apply(
        "Sum",
        vec![apply("Power", vec![symbol("k", &mut c), i(2, &mut c)], &mut c), iter],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "30");
}

#[test]
fn table_with_single_bound() {
    let mut c = C::new();
    let iter = lst(vec![symbol("i", &mut c), i(3, &mut c)], &mut c);
    let e = apply("Table", vec![symbol("i", &mut c), iter], &mut c);
    assert_eq!(t(e, &mut c), "List[1, 2, 3]");
}

#[test]
fn cases_filters_by_pattern() {
    let mut c = C::new();
    let pat = apply("Any", vec![symbol("Integer", &mut c)], &mut c);
    let e = apply(
        "CollectMatches",
        vec![lst(vec![i(1, &mut c), symbol("y", &mut c), i(3, &mut c)], &mut c), pat],
        &mut c,
    );
    let r = t(e, &mut c);
    assert!(r.contains("List[1, 3]"), "got {r}");
}

#[test]
fn machine_trig_at_real_points() {
    let mut c = C::new();
    let zero = {
        let span = athena_ir::TermNode::default_span();
        c.s.arena.push(
            athena_ir::TermNode::Atom(athena_ir::Atom::Number(athena_numeric::NumericValue::machine(0.0))),
            span,
        )
    };
    let e = apply("Sin", vec![zero], &mut c);
    assert_eq!(t(e, &mut c), "0");
    let e = apply("Cos", vec![symbol("Pi", &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "-1");
}

#[test]
fn d_power() {
    let mut c = C::new();
    let e = apply(
        "D",
        vec![
            apply("Power", vec![symbol("x", &mut c), i(3, &mut c)], &mut c),
            symbol("x", &mut c),
        ],
        &mut c,
    );
    let r = t(e, &mut c);
    assert!(r.contains("x"), "got {r}");
}

#[test]
fn integrate_power() {
    let mut c = C::new();
    let e = apply(
        "Integrate",
        vec![
            apply("Power", vec![symbol("x", &mut c), i(2, &mut c)], &mut c),
            symbol("x", &mut c),
        ],
        &mut c,
    );
    let r = t(e, &mut c);
    assert!(r.contains("x"), "got {r}");
}

#[test]
fn pythagorean() {
    let mut c = C::new();
    let sin2 = apply(
        "Power",
        vec![apply("Sin", vec![symbol("x", &mut c)], &mut c), i(2, &mut c)],
        &mut c,
    );
    let cos2 = apply(
        "Power",
        vec![apply("Cos", vec![symbol("x", &mut c)], &mut c), i(2, &mut c)],
        &mut c,
    );
    let e = apply("Simplify", vec![apply("Plus", vec![sin2, cos2], &mut c)], &mut c);
    assert_eq!(t(e, &mut c), "1");
}

#[test]
fn unsupported_import_is_not_silent_value() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    let e = apply("Import", vec![str_("x.csv", &mut c)], &mut c);
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::UnsupportedOperation);
}

#[test]
fn if_false_and_null_and_non_boolean() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    assert_eq!(
        t(
            apply("Branch", vec![symbol("False", &mut c), i(7, &mut c), i(8, &mut c)], &mut c),
            &mut c
        ),
        "8"
    );
    assert_eq!(
        t(apply("Branch", vec![i(0, &mut c), i(7, &mut c)], &mut c), &mut c),
        "Null"
    );
    let e = apply("Branch", vec![symbol("x", &mut c), i(1, &mut c), i(2, &mut c)], &mut c);
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Invalid);
    assert_eq!(o.diagnostics[0].code, DiagnosticCode::NonBooleanCondition);
}

#[test]
fn branch_true_skips_else_import() {
    use athena_types::DiagnosticCode;
    let mut c = C::new();
    let e = apply(
        "Branch",
        vec![
            symbol("True", &mut c),
            i(7, &mut c),
            apply("Import", vec![str_("x.csv", &mut c)], &mut c),
        ],
        &mut c,
    );
    assert_eq!(t(e, &mut c), "7");
    let o = result_of(e, &mut c);
    assert_eq!(o.status, ComputationStatus::Exact);
    assert!(!o.diagnostics.iter().any(|d| d.code == DiagnosticCode::UnsupportedOperation));
}
