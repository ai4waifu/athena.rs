//! Interp 执行层语义验收（Living `25` L2 · KernelIR + VM）。

use athena_engine::{
    diagnostics::term_summary::term_debug,
    execution,
    execution::vm::evaluate_session,
    runtime::{
        Session,
        values::arena::{push_app_named, push_int, push_list, push_symbol_name},
    },
};

type Tid = athena_types::TermId;

fn eval(s: &mut Session, expr: Tid) -> String {
    let out = evaluate_session(s, expr);
    term_debug(s, out.term)
}

fn sym(name: &str, s: &mut Session) -> Tid {
    push_symbol_name(s, name)
}

fn int(n: i64, s: &mut Session) -> Tid {
    push_int(s, n)
}

fn list(items: Vec<Tid>, s: &mut Session) -> Tid {
    push_list(s, items)
}

fn app(head: &str, args: Vec<Tid>, s: &mut Session) -> Tid {
    push_app_named(s, head, args)
}

#[test]
fn arithmetic_normalization() {
    let mut s = Session::new();
    // 2 + 3 → 5
    let e = app("Plus", vec![int(2, &mut s), int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "5");
    // x + 2 + x → 2 x + 2（系数合并）
    let e = app("Plus", vec![sym("x", &mut s), int(2, &mut s), sym("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Times[2, x]"), "got {r}");
    // 2 * 3 * x → 6 x
    let e = app("Times", vec![int(2, &mut s), int(3, &mut s), sym("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("6"), "got {r}");
    // (1 + 2) * x → 3 x（分配）
    let sum = app("Plus", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = app("Times", vec![sum, sym("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Times[3, x]"), "got {r}");
    // x^0 → 1
    let e = app("Power", vec![sym("x", &mut s), int(0, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "1");
    // 2^10 → 1024
    let e = app("Power", vec![int(2, &mut s), int(10, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "1024");
    // 未知算子惰性重建
    let e = app("Foo", vec![int(1, &mut s), sym("y", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "Foo[1, y]");
    // 精确三角
    let e = app("Cos", vec![sym("Pi", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "-1");
    let e = app("Sin", vec![int(0, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "0");
    // Sqrt[4] → 2
    let e = app("Sqrt", vec![int(4, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "2");
}

#[test]
fn comparisons_and_logic() {
    let mut s = Session::new();
    let e = app("Less", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let e = app("Equal", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "False");
    // 比较链：1 < 2 < 3（嵌套未求值形式）
    let nested = app("Less", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = app("Less", vec![nested, int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let t = sym("True", &mut s);
    let f = sym("False", &mut s);
    let e = app("And", vec![t, f], &mut s);
    assert_eq!(eval(&mut s, e), "False");
    let t = sym("True", &mut s);
    let f = sym("False", &mut s);
    let e = app("Or", vec![t, f], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let f = sym("False", &mut s);
    let e = app("Not", vec![f], &mut s);
    assert_eq!(eval(&mut s, e), "True");
}

#[test]
fn if_and_which() {
    let mut s = Session::new();
    let cond = app("Less", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = app("If", vec![cond, int(10, &mut s), int(20, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "10");
    let cond = app("Greater", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = app("If", vec![cond, int(10, &mut s), int(20, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "20");
    let c1 = app("Equal", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let c2 = app("Equal", vec![int(3, &mut s), int(3, &mut s)], &mut s);
    let e = app("Which", vec![c1, int(0, &mut s), c2, int(42, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "42");
}

#[test]
fn set_and_compound_persist_across_evaluations() {
    let mut s = Session::new();
    let x = sym("x", &mut s);
    let set = app("Set", vec![x, int(5, &mut s)], &mut s);
    assert_eq!(eval(&mut s, set), "5");
    let e = app("Plus", vec![sym("x", &mut s), int(1, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "6");
    // Compound 语句序列：y = 2; y + 40
    let y = sym("y", &mut s);
    let set = app("Set", vec![y, int(2, &mut s)], &mut s);
    let plus = app("Plus", vec![sym("y", &mut s), int(40, &mut s)], &mut s);
    let e = app("CompoundExpression", vec![set, plus], &mut s);
    assert_eq!(eval(&mut s, e), "42");
}

#[test]
fn while_accumulator() {
    let mut s = Session::new();
    // s = 0; i = 1; While[i <= 3, i = i + 1; s = s + i]; s  → 9
    let set0 = app("Set", vec![sym("s", &mut s), int(0, &mut s)], &mut s);
    let set_i_init = app("Set", vec![sym("i", &mut s), int(1, &mut s)], &mut s);
    let cond = app("LessEqual", vec![sym("i", &mut s), int(3, &mut s)], &mut s);
    let plus1 = app("Plus", vec![sym("i", &mut s), int(1, &mut s)], &mut s);
    let set_i = app("Set", vec![sym("i", &mut s), plus1], &mut s);
    let plus2 = app("Plus", vec![sym("s", &mut s), sym("i", &mut s)], &mut s);
    let set_s = app("Set", vec![sym("s", &mut s), plus2], &mut s);
    let body = app("CompoundExpression", vec![set_i, set_s], &mut s);
    let wh = app("While", vec![cond, body], &mut s);
    let e = app("CompoundExpression", vec![set0, set_i_init, wh, sym("s", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "9");
}

#[test]
fn table_sum_and_module() {
    let mut s = Session::new();
    // Table[i^2, {i, 1, 4}] → {1, 4, 9, 16}
    let sq = app("Power", vec![sym("i", &mut s), int(2, &mut s)], &mut s);
    let iter = list(vec![sym("i", &mut s), int(1, &mut s), int(4, &mut s)], &mut s);
    let e = app("Table", vec![sq, iter], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[1, 4, 9, 16]"), "got {r}");
    // Sum[k, {k, 1, 10}] → 55
    let iter = list(vec![sym("k", &mut s), int(1, &mut s), int(10, &mut s)], &mut s);
    let e = app("Sum", vec![sym("k", &mut s), iter], &mut s);
    assert_eq!(eval(&mut s, e), "55");
    // With[{x = 3}, x + 1] → 4
    let locals = list(vec![app("Set", vec![sym("x", &mut s), int(3, &mut s)], &mut s)], &mut s);
    let body = app("Plus", vec![sym("x", &mut s), int(1, &mut s)], &mut s);
    let e = app("With", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "4");
    // Module[{x}, x] → 唯一化符号（含 `$`）
    let locals = list(vec![sym("x", &mut s)], &mut s);
    let e = app("Module", vec![locals, sym("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("x$"), "got {r}");
    // Module[{x}, x = 1; x + 1]：legacy 桥的 fresh-env quirk — x=1 写入 fresh env，
    // 体中 x 先被物化为唯一化符号，结果保持 `Plus[1, x$N]` 形态（与 legacy 一致）。
    let locals = list(vec![sym("x", &mut s)], &mut s);
    let set = app("Set", vec![sym("x", &mut s), int(1, &mut s)], &mut s);
    let body = app("Plus", vec![sym("x", &mut s), int(1, &mut s)], &mut s);
    let body = app("CompoundExpression", vec![set, body], &mut s);
    let e = app("Module", vec![locals, body], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Plus[1, x$"), "got {r}");
    // Module[{x = 1}, x + 1] → 2（初始化局部，legacy 合同）
    let locals = list(vec![app("Set", vec![sym("x", &mut s), int(1, &mut s)], &mut s)], &mut s);
    let body = app("Plus", vec![sym("x", &mut s), int(1, &mut s)], &mut s);
    let e = app("Module", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "2");
}

#[test]
fn downvalues_and_match_q() {
    let mut s = Session::new();
    // f[x_] := x^2
    let xv = sym("x", &mut s);
    let blank = app("Blank", vec![], &mut s);
    let pat = app("Pattern", vec![xv, blank], &mut s);
    let lhs = app("f", vec![pat], &mut s);
    let rhs = app("Power", vec![sym("x", &mut s), int(2, &mut s)], &mut s);
    let def = app("SetDelayed", vec![lhs, rhs], &mut s);
    assert_eq!(eval(&mut s, def), "Null");
    // f[3] → 9
    let call = app("f", vec![int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, call), "9");
    // MatchQ[3, _Integer] → True
    let blank_int = app("Blank", vec![sym("Integer", &mut s)], &mut s);
    let e = app("MatchQ", vec![int(3, &mut s), blank_int], &mut s);
    assert_eq!(eval(&mut s, e), "True");
}

#[test]
fn part_span_and_functions() {
    let mut s = Session::new();
    // Part[{a, b, c}, 2] → b
    let l = list(vec![sym("a", &mut s), sym("b", &mut s), sym("c", &mut s)], &mut s);
    let e = app("Part", vec![l, int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "b");
    // Span[1, 3] → {1, 2, 3}
    let e = app("Span", vec![int(1, &mut s), int(3, &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[1, 2, 3]"), "got {r}");
    // Function[x, x + 1][4] → 5
    let body = app("Plus", vec![sym("x", &mut s), int(1, &mut s)], &mut s);
    let f = app("Function", vec![sym("x", &mut s), body], &mut s);
    let e = app("Application", vec![f, int(4, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "5");
}

#[test]
fn linear_algebra_paths() {
    let mut s = Session::new();
    // Det[{{1, 2}, {3, 4}}] → -2
    let m = list(
        vec![list(vec![int(1, &mut s), int(2, &mut s)], &mut s), list(vec![int(3, &mut s), int(4, &mut s)], &mut s)],
        &mut s,
    );
    let e = app("Det", vec![m], &mut s);
    assert_eq!(eval(&mut s, e), "-2");
    // LinearSolve[{{2, 0}, {0, 2}}, {{4}, {6}}] → {{2}, {3}}（列向量形态）
    let m = list(
        vec![list(vec![int(2, &mut s), int(0, &mut s)], &mut s), list(vec![int(0, &mut s), int(2, &mut s)], &mut s)],
        &mut s,
    );
    let rhs = list(vec![list(vec![int(4, &mut s)], &mut s), list(vec![int(6, &mut s)], &mut s)], &mut s);
    let e = app("LinearSolve", vec![m, rhs], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[List[2], List[3]]"), "got {r}");
    // 行向量 b 为 legacy 合同下的未求值回显形态
    let rhs_row = list(vec![int(4, &mut s), int(6, &mut s)], &mut s);
    let e = app("LinearSolve", vec![m, rhs_row], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("LinearSolve["), "got {r}");
}

#[test]
fn interp_outcome_exposed() {
    let mut s = Session::new();
    let e = app("Plus", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let out: execution::Outcome = evaluate_session(&mut s, e);
    assert!(matches!(out.kind, execution::EvalKind::Value));
    assert!(out.diagnostics.is_empty());
}
