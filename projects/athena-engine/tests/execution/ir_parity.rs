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
