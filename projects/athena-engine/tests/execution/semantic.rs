//! 执行层语义验收（Living `25` L2 · `ExecutionIR` via `evaluate_term`）。

use athena_engine::{
    api::request::{AthenaRequest, ControlPlan},
    diagnostics::term_summary::term_debug,
    execution,
    execution::evaluate_term,
    runtime::{
        Session,
        values::arena::{push_application_named, push_int, push_list, push_semantic, push_symbol_name},
    },
};
use athena_types::BindingEvaluationPolicy;
use athena_ir::SemanticOperator;

type Tid = athena_types::TermId;

fn eval(s: &mut Session, expr: Tid) -> String {
    let out = evaluate_term(s, expr);
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


fn sem(op: SemanticOperator, args: Vec<Tid>, s: &mut Session) -> Tid {
    push_semantic(s, op, args)
}

fn ext(head: &str, args: Vec<Tid>, s: &mut Session) -> Tid {
    push_application_named(s, head, args)
}


#[test]
fn arithmetic_normalization() {
    let mut s = Session::new();
    // 2 + 3 → 5
    let e = sem(SemanticOperator::Add, vec![int(2, &mut s), int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "5");
    // x + 2 + x → 2 x + 2（系数合并）
    let e = sem(SemanticOperator::Add, vec![symbol("x", &mut s), int(2, &mut s), symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Multiply[2, x]"), "got {r}");
    // 2 * 3 * x → 6 x
    let e = sem(SemanticOperator::Multiply, vec![int(2, &mut s), int(3, &mut s), symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("6"), "got {r}");
    // (1 + 2) * x → 3 x（分配）
    let sum = sem(SemanticOperator::Add, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = sem(SemanticOperator::Multiply, vec![sum, symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("Multiply[3, x]"), "got {r}");
    // x^0 → 1
    let e = sem(SemanticOperator::Power, vec![symbol("x", &mut s), int(0, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "1");
    // 2^10 → 1024
    let e = sem(SemanticOperator::Power, vec![int(2, &mut s), int(10, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "1024");
    // 未知算子惰性重建
    let e = ext("Foo", vec![int(1, &mut s), symbol("y", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "Foo[1, y]");
    // 精确三角
    let e = ext("Cos", vec![symbol("Pi", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "-1");
    let e = ext("Sin", vec![int(0, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "0");
    // Sqrt[4] → 2
    let e = sem(SemanticOperator::Sqrt, vec![int(4, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "2");
}

#[test]
fn comparisons_and_logic() {
    let mut s = Session::new();
    let e = sem(SemanticOperator::Less, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let e = sem(SemanticOperator::Equal, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "False");
    // 比较链：1 < 2 < 3（嵌套未求值形式）
    let nested = sem(SemanticOperator::Less, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = sem(SemanticOperator::Less, vec![nested, int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let t = symbol("True", &mut s);
    let f = symbol("False", &mut s);
    let e = sem(SemanticOperator::And, vec![t, f], &mut s);
    assert_eq!(eval(&mut s, e), "False");
    let t = symbol("True", &mut s);
    let f = symbol("False", &mut s);
    let e = sem(SemanticOperator::Or, vec![t, f], &mut s);
    assert_eq!(eval(&mut s, e), "True");
    let f = symbol("False", &mut s);
    let e = sem(SemanticOperator::Not, vec![f], &mut s);
    assert_eq!(eval(&mut s, e), "True");
}

#[test]
fn if_and_which() {
    let mut s = Session::new();
    let cond = sem(SemanticOperator::Less, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = ext("Branch", vec![cond, int(10, &mut s), int(20, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "10");
    let cond = sem(SemanticOperator::Greater, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let e = ext("Branch", vec![cond, int(10, &mut s), int(20, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "20");
    let c1 = sem(SemanticOperator::Equal, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let c2 = sem(SemanticOperator::Equal, vec![int(3, &mut s), int(3, &mut s)], &mut s);
    let e = ext("Cond", vec![c1, int(0, &mut s), c2, int(42, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "42");
}

#[test]
fn set_and_compound_persist_across_evaluations() {
    let mut s = Session::new();
    let x = symbol("x", &mut s);
    let set = ext("Define", vec![x, int(5, &mut s)], &mut s);
    assert_eq!(eval(&mut s, set), "5");
    let e = sem(SemanticOperator::Add, vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "6");
    // Compound 语句序列：y = 2; y + 40
    let y = symbol("y", &mut s);
    let set = ext("Define", vec![y, int(2, &mut s)], &mut s);
    let plus = sem(SemanticOperator::Add, vec![symbol("y", &mut s), int(40, &mut s)], &mut s);
    let e = ext("Sequence", vec![set, plus], &mut s);
    assert_eq!(eval(&mut s, e), "42");
}

#[test]
fn while_accumulator() {
    let mut s = Session::new();
    // s = 0; i = 1; While[i <= 3, i = i + 1; s = s + i]; s  → 9
    let set0 = ext("Define", vec![symbol("s", &mut s), int(0, &mut s)], &mut s);
    let set_i_init = ext("Define", vec![symbol("i", &mut s), int(1, &mut s)], &mut s);
    let cond = sem(SemanticOperator::LessEqual, vec![symbol("i", &mut s), int(3, &mut s)], &mut s);
    let plus1 = sem(SemanticOperator::Add, vec![symbol("i", &mut s), int(1, &mut s)], &mut s);
    let set_i = ext("Define", vec![symbol("i", &mut s), plus1], &mut s);
    let plus2 = sem(SemanticOperator::Add, vec![symbol("s", &mut s), symbol("i", &mut s)], &mut s);
    let set_s = ext("Define", vec![symbol("s", &mut s), plus2], &mut s);
    let body = ext("Sequence", vec![set_i, set_s], &mut s);
    let wh = ext("LoopWhile", vec![cond, body], &mut s);
    let e = ext("Sequence", vec![set0, set_i_init, wh, symbol("s", &mut s)], &mut s);
    assert_eq!(eval(&mut s, e), "9");
}

fn eval_request(s: &mut Session, request: AthenaRequest) -> String {
    let result_id = execution::execute_ir_request(s, request).expect("ir");
    let term = s.results.get(result_id).and_then(|r| r.symbolic_term).expect("term");
    term_debug(s, term)
}

#[test]
fn iterate_collects_ordered_collection() {
    let mut s = Session::new();
    let binder = symbol("i", &mut s);
    let range = list(vec![int(1, &mut s), int(2, &mut s), int(3, &mut s), int(4, &mut s)], &mut s);
    let body = sem(SemanticOperator::Power, vec![binder, int(2, &mut s)], &mut s);
    let request = AthenaRequest::Control(ControlPlan::Iterate {
        binder,
        range,
        body: Box::new(AthenaRequest::Term(body)),
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    assert_eq!(eval_request(&mut s, request), "List[1, 4, 9, 16]");
}

#[test]
fn table_sum_and_module() {
    let mut s = Session::new();
    let iter = list(vec![symbol("k", &mut s), int(1, &mut s), int(10, &mut s)], &mut s);
    let e = sem(SemanticOperator::Sum, vec![symbol("k", &mut s), iter], &mut s);
    assert_eq!(eval(&mut s, e), "55");
    // LocalScope[{x = 3}, x + 1] → 4
    let locals = list(vec![ext("Define", vec![symbol("x", &mut s), int(3, &mut s)], &mut s)], &mut s);
    let body = sem(SemanticOperator::Add, vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let e = ext("LocalScope", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "4");
    // LexicalScope[{x}, x] → 唯一化符号（含 `$`）
    let locals = list(vec![symbol("x", &mut s)], &mut s);
    let e = ext("LexicalScope", vec![locals, symbol("x", &mut s)], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("x$"), "got {r}");
    // LexicalScope[{x}, x = 1; x + 1] → 2（局部 Define 写入当前 ScopeFrame）。
    let locals = list(vec![symbol("x", &mut s)], &mut s);
    let set = ext("Define", vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let body = sem(SemanticOperator::Add, vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let body = ext("Sequence", vec![set, body], &mut s);
    let e = ext("LexicalScope", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "2");
    // LexicalScope[{x = 1}, x + 1] → 2（初始化局部）
    let locals = list(vec![ext("Define", vec![symbol("x", &mut s), int(1, &mut s)], &mut s)], &mut s);
    let body = sem(SemanticOperator::Add, vec![symbol("x", &mut s), int(1, &mut s)], &mut s);
    let e = ext("LexicalScope", vec![locals, body], &mut s);
    assert_eq!(eval(&mut s, e), "2");
}

#[test]
fn downvalues_and_match_q() {
    use athena_engine::reasoning::trs::{PatternConstraint, TermPattern};
    use athena_ir::Atom;
    use athena_types::ValueTypeId;

    let mut s = Session::new();
    let x_term = symbol("x", &mut s);
    let x_sym = match s.arena.get(x_term) {
        Some(athena_ir::TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let f_op = s.operators.intern("f");
    let f_sym = s.arena.symbols_mut().intern("f");
    let rhs = sem(SemanticOperator::Power, vec![symbol("x", &mut s), int(2, &mut s)], &mut s);
    let pattern = TermPattern::Application {
        operator: athena_ir::ApplicationHead::Extension(f_op),
        arguments: vec![TermPattern::Bind { name: x_sym, inner: Box::new(TermPattern::Any) }],
    };
    s.defs.register_rule(f_sym, pattern, rhs);
    let call = ext("f", vec![int(3, &mut s)], &mut s);
    assert_eq!(eval(&mut s, call), "9");

    let constrained = TermPattern::Constrained {
        pattern: Box::new(TermPattern::Any),
        constraint: PatternConstraint::ValueType(ValueTypeId::ExactInteger),
    };
    let mut binds = std::collections::HashMap::new();
    let three = int(3, &mut s);
    let y = symbol("y", &mut s);
    assert!(athena_engine::execution::builtins::patterns::match_term_pattern(&s, three, &constrained, &mut binds));
    assert!(!athena_engine::execution::builtins::patterns::match_term_pattern(&s, y, &constrained, &mut binds));
}


#[test]
fn linear_algebra_paths() {
    let mut s = Session::new();
    // Determinant[{{1, 2}, {3, 4}}] → -2
    let m = list(vec![list(vec![int(1, &mut s), int(2, &mut s)], &mut s), list(vec![int(3, &mut s), int(4, &mut s)], &mut s)], &mut s);
    let e = sem(SemanticOperator::Determinant, vec![m], &mut s);
    assert_eq!(eval(&mut s, e), "-2");
    // LinearSolve[{{2, 0}, {0, 2}}, {{4}, {6}}] → {{2}, {3}}（列向量形态）
    let m = list(vec![list(vec![int(2, &mut s), int(0, &mut s)], &mut s), list(vec![int(0, &mut s), int(2, &mut s)], &mut s)], &mut s);
    let rhs = list(vec![list(vec![int(4, &mut s)], &mut s), list(vec![int(6, &mut s)], &mut s)], &mut s);
    let e = ext("LinearSolve", vec![m, rhs], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("List[List[2], List[3]]"), "got {r}");
    // 行向量 b 为未求值回显形态
    let rhs_row = list(vec![int(4, &mut s), int(6, &mut s)], &mut s);
    let e = ext("LinearSolve", vec![m, rhs_row], &mut s);
    let r = eval(&mut s, e);
    assert!(r.contains("LinearSolve["), "got {r}");
}

#[test]
fn term_evaluation_exposed() {
    let mut s = Session::new();
    let e = sem(SemanticOperator::Add, vec![int(1, &mut s), int(2, &mut s)], &mut s);
    let out: execution::TermEvaluation = evaluate_term(&mut s, e);
    assert!(matches!(out.kind, execution::EvalKind::Value));
    assert!(out.diagnostics.is_empty());
}
