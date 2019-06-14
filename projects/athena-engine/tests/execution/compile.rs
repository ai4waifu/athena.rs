//! Interp 执行层语义验收（Living `25` L2 · KernelIR + VM）。

use athena_engine::{
    diagnostics::term_summary::term_debug,
    execution,
    execution::vm::evaluate_session,
    runtime::{
        Session,
        values::arena::{push_application_named, push_int, push_list, push_symbol_name},
    },
};

type Tid = athena_types::TermId;

fn eval(s: &mut Session, expr: Tid) -> String {
    let out = evaluate_session(s, expr);
    term_debug(s, out.term)
}

fn symbol(name: &str, s: &mut Session) -> Tid {
    push_symbol_name(s, name)
}

fn int(n: i64, s: &mut Session) -> Tid {
    push_int(s, n)
}

fn list(items: Vec<Tid>, s: &mut Session) -> Tid {
    push_list(s, items)
}

fn apply(head: &str, args: Vec<Tid>, s: &mut Session) -> Tid {
    push_application_named(s, head, args)
}

#[test]
fn arithmetic_normalization() {
    let mut s = Session::new();
    // 2 + 3 → 5
    let e = apply("Plus", vec![int(2, &mut s), int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "5");
    // x + 2 + x → 2 x + 2（系数合并）
    let e = apply("Plus", vec![symbol("x", &mut s), int(2, &mut s), symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Times[2, x]"), "got {r}");
    // 2 * 3 * x → 6 x
    let e = apply("Times", vec![int(2, &mut s), int(3, &mut s), symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("6"), "got {r}");
    // (1 + 2) * x → 3 x（分配）
    let sum = apply("Plus", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = apply("Times", vec![sum, symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Times[3, x]"), "got {r}");
    // x^0 → 1
    let e = apply("Power", vec![symbol("x", &mut s), int(0, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "1");
    // 2^10 → 1024
    let e = apply("Power", vec![int(2, &mut s), int(10, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "1024");
    // 未知算子惰性重建
    let e = apply("Foo", vec![int(1, &mut s), symbol("y", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "Foo[1, y]");
    // 精确三角
    let e = apply("Cos", vec![symbol("Pi", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "-1");
    let e = apply("Sin", vec![int(0, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "0");
    // Sqrt[4] → 2
    let e = apply("Sqrt", vec![int(4, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "2");
}

#[test]
fn comparisons_and_logic() {
    let mut s = Session::new();
    let e = apply("Less", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let e = apply("Equal", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "False");
    // 比较链：1 < 2 < 3（嵌套未求值形式）
    let nested = apply("Less", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = apply("Less", vec![nested, int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let t = symbol("True", &mut s);
    let f = symbol("False", &mut s);
    let e = apply("And", vec![t, f], &mut s);
    assert_eq!(eval(&mut s, e), "False");
    let t = symbol("True", &mut s);
    let f = symbol("False", &mut s);
    let e = apply("Or", vec![t, f], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let f = symbol("False", &mut s);
    let e = apply("Not", vec![f], &mut s);
    assert_eq!(eval(&mut s, e), "True");
}

#[test]
fn if_and_which() {
    let mut s = Session::new();
    let cond = apply("Less", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = apply("Branch", vec![cond, int(10, &mut s), int(20, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "10");
    let cond = apply("Greater", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = apply("Branch", vec![cond, int(10, &mut s), int(20, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "20");
    let c1 = apply("Equal", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let c2 = apply("Equal", vec![int(3, &mut s), int(3, &mut s)], &mut s);
    let e = apply("Cond", vec![c1, int(0, &mut s), c2, int(42, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "42");
}

#[test]
fn set_and_compound_persist_across_evaluations() {
    let mut s = Session::new();
    let x = symbol("x", &mut s);
    let set = apply("Define", vec![x, int(5, &mut s)], &mut s);
    assert_eq!(eval(&mut s, set), "5");
    let e = apply("Plus", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "6");
    // Compound 语句序列：y = 2; y + 40
    let y = symbol("y", &mut s);
    let set = apply("Define", vec![y, int(2, &mut s)], &mut s);
    let plus = apply("Plus", vec![symbol("y", &mut s), int(40, &mut s)], &mut s);
    let e = apply("Sequence", vec![set, plus], &mut s);
    assert_eq!(eval(&mut s, e), "42");
}

#[test]
fn while_accumulator() {
    let mut s = Session::new();
    // s = 0; i = 1; While[i <= 3, i = i + 1; s = s + i]; s  → 9
    let set0 = apply("Define", vec![symbol("s", &mut s), int(0, &mut s)], &mut s);
    let set_i_init = apply("Define", vec![symbol("i", &mut s), int(1, &mut s)], &mut s);
    let cond = apply("LessEqual", vec![symbol("i", &mut s), int(3, &mut s)], &mut s);
    let plus1 = apply("Plus", vec![symbol("i", &mut s), int(1, &mut s)], &mut s);
    let set_i = apply("Define", vec![symbol("i", &mut s), plus1], &mut s);
    let plus2 = apply("Plus", vec![symbol("s", &mut s), symbol("i", &mut s)], &mut s);
    let set_s = apply("Define", vec![symbol("s", &mut s), plus2], &mut s);
    let body = apply("Sequence", vec![set_i, set_s], &mut s);
    let wh = apply("LoopWhile", vec![cond, body], &mut s);
    let e = apply("Sequence", vec![set0, set_i_init, wh, symbol("s", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "9");
}

#[test]
fn table_sum_and_module() {
    let mut s = Session::new();
    // Table[i^2, {i, 1, 4}] → {1, 4, 9, 16}
    let sq = apply("Power", vec![symbol("i", &mut s), int(2, &mut s)], &mut s);
    let iter = list(vec![symbol("i", &mut s), int(1, &mut s), int(4, &mut s)], &mut s);
    let e = apply("Table", vec![sq, iter], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[1, 4, 9, 16]"), "got {r}");
    // Sum[k, {k, 1, 10}] → 55
    let iter = list(vec![symbol("k", &mut s), int(1, &mut s), int(10, &mut s)], &mut s);
    let e = apply("Sum", vec![symbol("k", &mut s), iter], &mut s);
    assert_eq!(eval(&mut s, e), "55");
    // LocalScope[{x = 3}, x + 1] → 4
    let locals = list(vec![apply("Define", vec![symbol("x", &mut s), int(3, &mut s)], &mut s)], &mut s);
    let body = apply("Plus", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let e = apply("LocalScope", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "4");
    // LexicalScope[{x}, x] → 唯一化符号（含 `$`）
    let locals = list(vec![symbol("x", &mut s)], &mut s);
    let e = apply("LexicalScope", vec![locals, symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("x$"), "got {r}");
    // LexicalScope[{x}, x = 1; x + 1]：fresh-env quirk — x=1 写入 fresh env，
    // 体中 x 先被物化为唯一化符号，结果保持 `Plus[1, x$N]` 形态。
    let locals = list(vec![symbol("x", &mut s)], &mut s);
    let set = apply("Define", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let body = apply("Plus", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let body = apply("Sequence", vec![set, body], &mut s);
    let e = apply("LexicalScope", vec![locals, body], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Plus[1, x$"), "got {r}");
    // LexicalScope[{x = 1}, x + 1] → 2（初始化局部）
    let locals = list(vec![apply("Define", vec![symbol("x", &mut s), int(1, &mut s)], &mut s)], &mut s);
    let body = apply("Plus", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let e = apply("LexicalScope", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "2");
}

#[test]
fn downvalues_and_match_q() {
    let mut s = Session::new();
    // f[x_] := x^2
    let xv = symbol("x", &mut s);
    let blank = apply("Any", vec![], &mut s);
    let pat = apply("Bind", vec![xv, blank], &mut s);
    let lhs = apply("f", vec![pat], &mut s);
    let rhs = apply("Power", vec![symbol("x", &mut s), int(2, &mut s)], &mut s);
    let def = apply("DefineDeferred", vec![lhs, rhs], &mut s);
    assert_eq!(eval(&mut s, def), "Null");
    // f[3] → 9
    let call = apply("f", vec![int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, call), "9");
    // Matches[3, Any[Integer]] → True
    let blank_int = apply("Any", vec![symbol("Integer", &mut s)], &mut s);
    let e = apply("Matches", vec![int(3, &mut s), blank_int], &mut s);
    assert_eq!(eval(&mut s, e), "True");
}

#[test]
fn part_span_and_functions() {
    let mut s = Session::new();
    // Part[{a, b, c}, 2] → b
    let l = list(vec![symbol("a", &mut s), symbol("b", &mut s), symbol("c", &mut s)], &mut s);
    let e = apply("Part", vec![l, int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "b");
    // Span[1, 3] → {1, 2, 3}
    let e = apply("Span", vec![int(1, &mut s), int(3, &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[1, 2, 3]"), "got {r}");
    // Function[x, x + 1][4] → 5
    let body = apply("Plus", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let f = apply("Function", vec![symbol("x", &mut s), body], &mut s);
    let e = apply("Application", vec![f, int(4, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "5");
}

#[test]
fn linear_algebra_paths() {
    let mut s = Session::new();
    // Det[{{1, 2}, {3, 4}}] → -2
    let m = list(vec![list(vec![int(1, &mut s), int(2, &mut s)], &mut s), list(vec![int(3, &mut s), int(4, &mut s)], &mut s)], &mut s);
    let e = apply("Det", vec![m], &mut s);
    assert_eq!(eval(&mut s, e), "-2");
    // LinearSolve[{{2, 0}, {0, 2}}, {{4}, {6}}] → {{2}, {3}}（列向量形态）
    let m = list(vec![list(vec![int(2, &mut s), int(0, &mut s)], &mut s), list(vec![int(0, &mut s), int(2, &mut s)], &mut s)], &mut s);
    let rhs = list(vec![list(vec![int(4, &mut s)], &mut s), list(vec![int(6, &mut s)], &mut s)], &mut s);
    let e = apply("LinearSolve", vec![m, rhs], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[List[2], List[3]]"), "got {r}");
    // 行向量 b 为 legacy 合同下的未求值回显形态
    let rhs_row = list(vec![int(4, &mut s), int(6, &mut s)], &mut s);
    let e = apply("LinearSolve", vec![m, rhs_row], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("LinearSolve["), "got {r}");
}

#[test]
fn interp_outcome_exposed() {
    let mut s = Session::new();
    let e = apply("Plus", vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let out: execution::TermEvaluation = evaluate_session(&mut s, e);
    assert!(matches!(out.kind, execution::EvalKind::Value));
    assert!(out.diagnostics.is_empty());
}
