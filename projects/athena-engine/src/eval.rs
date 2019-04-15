//! 引擎桥接 [`Term`] 上的内建求值。

use std::cmp::Ordering;

use athena_numeric::{
    Number, abs as num_abs, add as num_add, compare as num_compare, div as num_div, factorial as num_factorial, mul as num_mul,
    pow as num_pow, sqrt as num_sqrt, to_f64_lossy as num_to_f64_lossy,
};
use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, Result, Severity};

use crate::numeric_clone::{clone_number, clone_term, clone_terms};
use crate::term::{Atom, Term, number_from_term};

fn map_num<T>(r: Result<T>) -> Result<T> {
    r
}

/// 求值结果是否声称「已得到正常值」。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalKind {
    /// 已归约为正常值（可为符号正规形）。
    Value,
    /// 保留未求值形式，不得冒充成功 exact。
    Unevaluated,
}

/// 带 [`ComputationStatus`] 与结构化诊断的求值出口。
#[derive(Debug, PartialEq)]
pub struct EvalOutcome {
    /// 结果项（失败时可为原式或保守回显）。
    pub term: Term,
    /// 值 / 未求值区分。
    pub kind: EvalKind,
    /// 统一计算状态。
    pub status: ComputationStatus,
    /// 结构化诊断（可含 `UnsupportedOperation` / `InvalidIndex` 等）。
    pub diagnostics: Vec<Diagnostic>,
}

impl EvalOutcome {
    /// 精确值出口。
    pub fn value(term: Term) -> Self {
        Self {
            term,
            kind: EvalKind::Value,
            status: ComputationStatus::Exact,
            diagnostics: Vec::new(),
        }
    }

    /// 未求值保留。
    pub fn unevaluated(term: Term) -> Self {
        Self {
            term,
            kind: EvalKind::Unevaluated,
            status: ComputationStatus::Unknown,
            diagnostics: Vec::new(),
        }
    }

    /// 硬失败：带 Error 诊断，状态为 [`ComputationStatus::Invalid`]。
    pub fn invalid(term: Term, diagnostic: Diagnostic) -> Self {
        Self {
            term,
            kind: EvalKind::Unevaluated,
            status: ComputationStatus::Invalid,
            diagnostics: vec![diagnostic],
        }
    }

    /// 是否含 Error 级诊断。
    pub fn has_error(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// 成功值且无 Error 诊断时返回项，否则返回首个 Error。
    pub fn into_checked(self) -> Result<Term> {
        if let Some(err) = self.diagnostics.into_iter().find(|d| d.severity == Severity::Error) {
            return Err(err);
        }
        Ok(self.term)
    }
}

/// 构造 `UnsupportedOperation` 诊断。
pub fn unsupported_operation(operation: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", operation)
}

/// 构造非法下标诊断。
pub fn invalid_index_diagnostic(index: i64, length: Option<u64>) -> Diagnostic {
    let d = Diagnostic::new(DiagnosticCode::InvalidIndex).arg("index", index);
    match length {
        Some(len) => d.arg("length", len),
        None => d,
    }
}

/// 构造非布尔条件诊断。
pub fn non_boolean_condition_diagnostic(got: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NonBooleanCondition)
        .detail("expected", "Boolean")
        .detail("got", got)
}

/// 将项解释为 typed Boolean。`True`/`False` 与精确 `1`/`0` 可接受。其它返回 `None`。
pub fn as_boolean(expr: &Term) -> Option<bool> {
    match expr {
        Term::Atom(Atom::Symbol(s)) if s == "True" => Some(true),
        Term::Atom(Atom::Symbol(s)) if s == "False" => Some(false),
        other => number_from_term(other).and_then(|n| {
            if n.is_zero() {
                Some(false)
            } else if *n == Number::small_int(1) {
                Some(true)
            } else {
                None
            }
        }),
    }
}

/// 在内建定义下求值表达式。未知头部保留为 Application。
pub fn evaluate(expr: &Term) -> Term {
    evaluate_outcome(expr).term
}

/// 求值并返回状态 / 诊断。公共输入失败不 panic。
pub fn evaluate_outcome(expr: &Term) -> EvalOutcome {
    evaluate_depth_outcome(expr, 0)
}

/// 求值：遇 Error 诊断则 `Err`，否则 `Ok(term)`。
pub fn evaluate_checked(expr: &Term) -> Result<Term> {
    evaluate_outcome(expr).into_checked()
}

fn evaluate_depth(expr: &Term, depth: u32) -> Term {
    evaluate_depth_outcome(expr, depth).term
}

fn evaluate_depth_outcome(expr: &Term, depth: u32) -> EvalOutcome {
    if depth > 256 {
        return EvalOutcome::unevaluated(clone_term(expr));
    }
    match expr {
        Term::Atom(_) => EvalOutcome::value(clone_term(expr)),
        Term::List(items) => {
            let mut diagnostics = Vec::new();
            let mut out = Vec::with_capacity(items.len());
            let mut all_value = true;
            for item in items {
                let o = evaluate_depth_outcome(item, depth + 1);
                if o.kind != EvalKind::Value {
                    all_value = false;
                }
                diagnostics.extend(o.diagnostics);
                out.push(o.term);
            }
            let term = Term::List(out);
            if diagnostics.iter().any(|d| d.severity == Severity::Error) {
                EvalOutcome {
                    term,
                    kind: EvalKind::Unevaluated,
                    status: ComputationStatus::Invalid,
                    diagnostics,
                }
            } else if all_value {
                EvalOutcome {
                    term,
                    kind: EvalKind::Value,
                    status: ComputationStatus::Exact,
                    diagnostics,
                }
            } else {
                EvalOutcome {
                    term,
                    kind: EvalKind::Unevaluated,
                    status: ComputationStatus::Partial,
                    diagnostics,
                }
            }
        }
        Term::Application { head, arguments: args } => {
            let head_o = evaluate_depth_outcome(head, depth + 1);
            let mut diagnostics = head_o.diagnostics;
            let mut args_e = Vec::with_capacity(args.len());
            for a in args {
                let o = evaluate_depth_outcome(a, depth + 1);
                diagnostics.extend(o.diagnostics);
                args_e.push(o.term);
            }
            let mut out = apply_builtin_outcome(&head_o.term, args_e, depth);
            if !diagnostics.is_empty() {
                diagnostics.append(&mut out.diagnostics);
                out.diagnostics = diagnostics;
                if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                    out.status = ComputationStatus::Invalid;
                    out.kind = EvalKind::Unevaluated;
                }
            }
            out
        }
    }
}

fn apply_builtin(head: &Term, args: Vec<Term>, depth: u32) -> Term {
    apply_builtin_outcome(head, args, depth).term
}

fn apply_builtin_outcome(head: &Term, args: Vec<Term>, depth: u32) -> EvalOutcome {
    let name = match head {
        Term::Atom(Atom::Symbol(s)) => s.as_str(),
        _ => {
            return EvalOutcome::unevaluated(Term::Application {
                head: Box::new(clone_term(head)),
                arguments: args,
            });
        }
    };

    match name {
        "Plus" => EvalOutcome::value(eval_plus(args)),
        "Times" => EvalOutcome::value(eval_times(args)),
        "Power" if args.len() == 2 => EvalOutcome::value(eval_power(clone_term(&args[0]), clone_term(&args[1]))),
        "Subtract" if args.len() == 2 => EvalOutcome::value(eval_plus(vec![
            clone_term(&args[0]),
            eval_times(vec![Term::int(-1), clone_term(&args[1])]),
        ])),
        "Divide" if args.len() == 2 => EvalOutcome::value(eval_times(vec![
            clone_term(&args[0]),
            eval_power(clone_term(&args[1]), Term::int(-1)),
        ])),
        "List" => EvalOutcome::value(Term::List(args)),
        "Simplify" if args.len() == 1 => EvalOutcome::value(eval_simplify(&args[0], depth)),
        "Sin" | "Cos" | "Tan" | "Exp" | "Log" if args.len() == 1 => {
            EvalOutcome::value(eval_machine_unary(name, &args[0]))
        }
        "Sqrt" if args.len() == 1 => EvalOutcome::value(eval_sqrt(&args[0])),
        "Abs" if args.len() == 1 => EvalOutcome::value(eval_abs(&args[0])),
        "Factorial" if args.len() == 1 => EvalOutcome::value(eval_factorial(&args[0])),
        "Map" if args.len() == 2 => EvalOutcome::value(eval_map(&args[0], &args[1], depth)),
        "Equal" if args.len() == 2 => {
            wrap_compare(eval_compare("Equal", &args[0], &args[1], |o| o == Ordering::Equal), "Equal")
        }
        "Unequal" if args.len() == 2 => {
            wrap_compare(eval_compare("Unequal", &args[0], &args[1], |o| o != Ordering::Equal), "Unequal")
        }
        "Less" if args.len() == 2 => {
            wrap_compare(eval_compare("Less", &args[0], &args[1], |o| o == Ordering::Less), "Less")
        }
        "Greater" if args.len() == 2 => {
            wrap_compare(eval_compare("Greater", &args[0], &args[1], |o| o == Ordering::Greater), "Greater")
        }
        "LessEqual" if args.len() == 2 => {
            wrap_compare(eval_compare("LessEqual", &args[0], &args[1], |o| o != Ordering::Greater), "LessEqual")
        }
        "GreaterEqual" if args.len() == 2 => {
            wrap_compare(eval_compare("GreaterEqual", &args[0], &args[1], |o| o != Ordering::Less), "GreaterEqual")
        }
        "And" if args.len() == 2 => wrap_logic(eval_logic_and(&args[0], &args[1]), "And"),
        "Or" if args.len() == 2 => wrap_logic(eval_logic_or(&args[0], &args[1]), "Or"),
        "Not" if args.len() == 1 => wrap_logic(eval_logic_not(&args[0]), "Not"),
        "Set" | "SetDelayed" if args.len() == 2 => evaluate_depth_outcome(&args[1], depth + 1),
        "D" | "Integrate" | "Limit" | "Series" | "DSolve" | "LaplaceTransform" => {
            let term = Term::Application {
                head: Box::new(clone_term(head)),
                arguments: args,
            };
            if let Some(req) = crate::calculus::try_calculus_request(&term) {
                let result = crate::calculus::execute_calculus(req);
                return EvalOutcome::value(crate::calculus::calculus_result_bridge_term(&result));
            }
            EvalOutcome::unevaluated(term)
        }
        "CompoundExpression" if !args.is_empty() => evaluate_depth_outcome(args.last().unwrap(), depth + 1),
        "Function" => EvalOutcome::unevaluated(Term::Application {
            head: Box::new(Term::symbol("Function")),
            arguments: args,
        }),
        "ReplaceAll" if args.len() == 2 => EvalOutcome::value(eval_replace_all(&args[0], &args[1], depth)),
        "Part" if args.len() == 2 => eval_part_outcome(&args[0], &args[1]),
        "Rule" | "RuleDelayed" if args.len() == 2 => EvalOutcome::unevaluated(Term::Application {
            head: Box::new(clone_term(head)),
            arguments: args,
        }),
        "Import" | "Export" | "Clear" | "Timing" => {
            let term = Term::Application {
                head: Box::new(Term::symbol(name)),
                arguments: args,
            };
            EvalOutcome::invalid(term, unsupported_operation(name))
        }
        _ => {
            if let Term::Application { head: fh, arguments: fargs } = head {
                if fh.is_symbol("Function") && fargs.len() == 1 && args.len() == 1 {
                    let body = substitute_slot(&fargs[0], &args[0]);
                    return evaluate_depth_outcome(&body, depth + 1);
                }
            }
            EvalOutcome::unevaluated(Term::Application {
                head: Box::new(clone_term(head)),
                arguments: args,
            })
        }
    }
}

fn wrap_compare(term: Term, head: &str) -> EvalOutcome {
    if matches!(&term, Term::Application { head: h, .. } if h.is_symbol(head)) {
        EvalOutcome::unevaluated(term)
    } else {
        EvalOutcome::value(term)
    }
}

fn wrap_logic(term: Term, head: &str) -> EvalOutcome {
    if matches!(&term, Term::Application { head: h, .. } if h.is_symbol(head)) {
        EvalOutcome::unevaluated(term)
    } else {
        EvalOutcome::value(term)
    }
}

fn eval_plus(args: Vec<Term>) -> Term {
    let mut flat = Vec::new();
    let mut sum: Option<Number> = None;
    for a in args {
        flatten_plus(a, &mut flat, &mut sum);
    }
    if let Some(s) = sum {
        if !s.is_zero() {
            flat.insert(0, Term::number(s));
        }
    }
    else if flat.is_empty() {
        return Term::int(0);
    }
    let flat = combine_like_plus_terms(flat);
    match flat.len() {
        0 => Term::int(0),
        1 => flat.into_iter().next().unwrap(),
        _ => Term::apply("Plus", flat),
    }
}

/// 合并 `c1·k + c2·k`（含裸 `k` 视为系数 1）。
fn combine_like_plus_terms(terms: Vec<Term>) -> Vec<Term> {
    let mut groups: Vec<(Term, Number)> = Vec::new();
    for t in terms {
        let (coef, kernel) = split_numeric_coeff(t);
        if let Some((_, acc)) = groups.iter_mut().find(|(k, _)| k == &kernel) {
            *acc = match num_add(clone_number(acc), coef) {
                Ok(v) => v,
                Err(_) => return groups_to_plus_terms(groups), // 回退：放弃合并
            };
        }
        else {
            groups.push((kernel, coef));
        }
    }
    groups_to_plus_terms(groups)
}

fn groups_to_plus_terms(groups: Vec<(Term, Number)>) -> Vec<Term> {
    groups
        .into_iter()
        .filter_map(|(kernel, coef)| {
            if coef.is_zero() {
                None
            }
            else if is_one_kernel(&kernel) {
                Some(Term::number(coef))
            }
            else if coef.is_one() {
                Some(kernel)
            }
            else {
                Some(eval_times(vec![Term::number(coef), kernel]))
            }
        })
        .collect()
}

fn is_one_kernel(term: &Term) -> bool {
    number_from_term(term).is_some_and(|n| n.is_one())
}

/// 拆出数值系数：`Times[c, rest…]` / 纯数 / 其它。
fn split_numeric_coeff(term: Term) -> (Number, Term) {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Times") && !args.is_empty() => {
            let mut coef = Number::small_int(1);
            let mut rest = Vec::new();
            for a in args {
                if let Some(n) = number_from_term(&a).map(clone_number) {
                    coef = num_mul(clone_number(&coef), n).unwrap_or(coef);
                }
                else {
                    rest.push(a);
                }
            }
            let kernel = match rest.len() {
                0 => Term::int(1),
                1 => rest.pop().unwrap(),
                _ => Term::apply("Times", rest),
            };
            (coef, kernel)
        }
        other => {
            if let Some(n) = number_from_term(&other).map(clone_number) {
                (n, Term::int(1))
            }
            else {
                (Number::small_int(1), other)
            }
        }
    }
}

fn flatten_plus(a: Term, flat: &mut Vec<Term>, sum: &mut Option<Number>) {
    match a {
        Term::Application { head, arguments: args } if head.is_symbol("Plus") => {
            for x in args {
                flatten_plus(x, flat, sum);
            }
        }
        other => push_plus_term(other, flat, sum),
    }
}

fn push_plus_term(a: Term, flat: &mut Vec<Term>, sum: &mut Option<Number>) {
    if let Some(n) = number_from_term(&a).map(clone_number) {
        *sum = Some(match sum.take() {
            Some(s) => map_num(num_add(clone_number(&s), n)).unwrap_or(s),
            None => n,
        });
    }
    else {
        flat.push(a);
    }
}

fn eval_times(args: Vec<Term>) -> Term {
    let mut flat = Vec::new();
    let mut prod: Option<Number> = None;
    for a in args {
        flatten_times(a, &mut flat, &mut prod);
    }
    if let Some(p) = prod {
        if p.is_zero() {
            return Term::int(0);
        }
        if !p.is_one() {
            flat.insert(0, Term::number(p));
        }
    }
    else if flat.is_empty() {
        return Term::int(1);
    }
    // Times 对 Plus 分配：c·(a+b) → c·a + c·b（仅单层，避免爆炸）
    if let Some(idx) = flat.iter().position(|t| matches!(t, Term::Application { head, .. } if head.is_symbol("Plus"))) {
        let plus = flat.remove(idx);
        if let Term::Application { arguments: summands, .. } = plus {
            let parts: Vec<Term> = summands
                .into_iter()
                .map(|s| {
                    let mut factors = clone_terms(&flat);
                    factors.push(s);
                    eval_times(factors)
                })
                .collect();
            return eval_plus(parts);
        }
    }
    let flat = canonicalize_times_factors(combine_like_powers(flat));
    match flat.len() {
        0 => Term::int(1),
        1 => flat.into_iter().next().unwrap(),
        _ => Term::apply("Times", flat),
    }
}

/// 数值系数前置，保持与历史 Times 规范一致。
fn canonicalize_times_factors(factors: Vec<Term>) -> Vec<Term> {
    let mut nums = Vec::new();
    let mut rest = Vec::new();
    for f in factors {
        if number_from_term(&f).is_some() {
            nums.push(f);
        }
        else {
            rest.push(f);
        }
    }
    nums.extend(rest);
    nums
}

/// 合并 `Power[b,e1] * Power[b,e2]`（裸符号视为 `Power[b,1]`）。
fn combine_like_powers(factors: Vec<Term>) -> Vec<Term> {
    let mut out: Vec<(Term, Term)> = Vec::new(); // (base, exp)
    let mut rest = Vec::new();
    for f in factors {
        let (base, exp) = match &f {
            Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 => {
                (clone_term(&args[0]), clone_term(&args[1]))
            }
            Term::Atom(Atom::Symbol(_)) => (clone_term(&f), Term::int(1)),
            _ => {
                rest.push(f);
                continue;
            }
        };
        if let Some((_, e)) = out.iter_mut().find(|(b, _)| b == &base) {
            *e = eval_plus(vec![clone_term(e), exp]);
        }
        else {
            out.push((base, exp));
        }
    }
    let mut merged: Vec<Term> = out
        .into_iter()
        .filter_map(|(base, exp)| {
            let p = eval_power(base, exp);
            if number_from_term(&p).is_some_and(|n| n.is_one()) { None } else { Some(p) }
        })
        .collect();
    merged.extend(rest);
    merged
}

fn flatten_times(a: Term, flat: &mut Vec<Term>, prod: &mut Option<Number>) {
    match a {
        Term::Application { head, arguments: args } if head.is_symbol("Times") => {
            for x in args {
                flatten_times(x, flat, prod);
            }
        }
        other => {
            if let Some(n) = number_from_term(&other).map(clone_number) {
                if n.is_zero() {
                    *prod = Some(Number::small_int(0));
                    return;
                }
                *prod = Some(match prod.take() {
                    Some(p) => map_num(num_mul(clone_number(&p), n)).unwrap_or(p),
                    None => n,
                });
            }
            else {
                flat.push(other);
            }
        }
    }
}

fn eval_power(base: Term, exp: Term) -> Term {
    if let Some(e) = number_from_term(&exp).map(clone_number) {
        if e.is_zero() {
            return Term::int(1);
        }
        if e.is_one() {
            return base;
        }
        if e.is_neg_one() {
            if let Some(b) = number_from_term(&base).map(clone_number) {
                if let Ok(v) = map_num(num_div(Number::small_int(1), b)) {
                    return Term::number(v);
                }
            }
        }
        // (c * u)^n → c^n * u^n（整数 n）
        if let Some(n) = e.as_integer_exp() {
            if let Term::Application { head, arguments: args } = &base {
                if head.is_symbol("Times") && args.len() >= 2 {
                    if let Some(c) = number_from_term(&args[0]).map(clone_number) {
                        if let Ok(cp) = map_num(num_pow(&c, &e)) {
                            let rest = if args.len() == 2 { clone_term(&args[1]) } else { Term::apply("Times", clone_terms(&args[1..])) };
                            return eval_times(vec![Term::number(cp), eval_power(rest, clone_term(&exp))]);
                        }
                    }
                }
            }
            // (u^a)^b → u^(a*b)（a,b 为数）
            if let Term::Application { head, arguments: args } = &base {
                if head.is_symbol("Power") && args.len() == 2 {
                    if let Some(a) = number_from_term(&args[1]).map(clone_number) {
                        if let Ok(ab) = map_num(num_mul(a, e)) {
                            return eval_power(clone_term(&args[0]), Term::number(ab));
                        }
                    }
                }
            }
            let _ = n;
        }
    }
    if let (Some(b), Some(e)) = (number_from_term(&base).map(clone_number), number_from_term(&exp).map(clone_number)) {
        if let Ok(v) = map_num(num_pow(&b, &e)) {
            return Term::number(v);
        }
    }
    Term::apply("Power", vec![base, exp])
}

fn eval_simplify(expr: &Term, depth: u32) -> Term {
    let e = evaluate_depth(expr, depth + 1);
    if let Some(one) = try_pythagorean(&e) {
        return one;
    }
    evaluate_depth(&e, depth + 1)
}

fn eval_machine_unary(name: &str, arg: &Term) -> Term {
    let Some(x) = number_from_term(arg).and_then(|n| num_to_f64_lossy(&n))
    else {
        return Term::apply(name, vec![clone_term(arg)]);
    };
    let y = match name {
        "Sin" => x.sin(),
        "Cos" => x.cos(),
        "Tan" => x.tan(),
        "Exp" => x.exp(),
        "Log" => x.ln(),
        _ => return Term::apply(name, vec![clone_term(arg)]),
    };
    if y.is_finite() { Term::real(y) } else { Term::apply(name, vec![clone_term(arg)]) }
}

fn eval_sqrt(arg: &Term) -> Term {
    if let Some(n) = number_from_term(arg).map(clone_number) {
        if let Ok(Some(v)) = map_num(num_sqrt(&n)) {
            return Term::number(v);
        }
    }
    Term::apply("Sqrt", vec![clone_term(arg)])
}

fn eval_abs(arg: &Term) -> Term {
    if let Some(n) = number_from_term(arg).map(clone_number) {
        return Term::number(num_abs(n));
    }
    Term::apply("Abs", vec![clone_term(arg)])
}

fn eval_factorial(arg: &Term) -> Term {
    if let Some(n) = number_from_term(arg).map(clone_number) {
        if let Ok(v) = map_num(num_factorial(&n)) {
            return Term::number(v);
        }
    }
    Term::apply("Factorial", vec![clone_term(arg)])
}

fn eval_compare<F>(head: &str, left: &Term, right: &Term, cmp: F) -> Term
where
    F: Fn(Ordering) -> bool,
{
    if let (Some(a), Some(b)) = (number_from_term(left), number_from_term(right)) {
        if let Some(ord) = num_compare(a, b) {
            return Term::int(if cmp(ord) { 1 } else { 0 });
        }
    }
    Term::apply(head, vec![clone_term(left), clone_term(right)])
}

fn eval_logic_and(left: &Term, right: &Term) -> Term {
    match (truthy(left), truthy(right)) {
        (Some(a), Some(b)) => Term::int(if a && b { 1 } else { 0 }),
        _ => Term::apply("And", vec![clone_term(left), clone_term(right)]),
    }
}

fn eval_logic_or(left: &Term, right: &Term) -> Term {
    match (truthy(left), truthy(right)) {
        (Some(a), Some(b)) => Term::int(if a || b { 1 } else { 0 }),
        _ => Term::apply("Or", vec![clone_term(left), clone_term(right)]),
    }
}

fn eval_logic_not(arg: &Term) -> Term {
    match truthy(arg) {
        Some(v) => Term::int(if v { 0 } else { 1 }),
        None => Term::apply("Not", vec![clone_term(arg)]),
    }
}

fn truthy(expr: &Term) -> Option<bool> {
    number_from_term(expr).map(Number::is_truthy)
}

fn eval_map(func: &Term, target: &Term, depth: u32) -> Term {
    let list = match target {
        Term::List(items) => items,
        other => return Term::apply("Map", vec![clone_term(func), clone_term(other)]),
    };
    Term::List(
        list.iter()
            .map(|item| {
                let mapped = map_one(func, item);
                evaluate_depth(&mapped, depth + 1)
            })
            .collect(),
    )
}

fn map_one(func: &Term, item: &Term) -> Term {
    match func {
        Term::Atom(Atom::Symbol(name)) => Term::apply(name.clone(), vec![clone_term(item)]),
        Term::Application { head, arguments: args } if head.is_symbol("Function") && args.len() == 1 => {
            substitute_slot(&args[0], item)
        }
        _ => Term::apply("Map", vec![clone_term(func), clone_term(item)]),
    }
}

/// Sin[x]^2 + Cos[x]^2 → 1（顺序可交换）。
fn try_pythagorean(expr: &Term) -> Option<Term> {
    let terms = match expr {
        Term::Application { head, arguments: args } if head.is_symbol("Plus") => args.as_slice(),
        _ => return None,
    };
    if terms.len() != 2 {
        return None;
    }
    let (a, b) = (&terms[0], &terms[1]);
    if is_trig_sq(a, "Sin") && is_trig_sq(b, "Cos") && same_trig_arg(a, b) {
        return Some(Term::int(1));
    }
    if is_trig_sq(a, "Cos") && is_trig_sq(b, "Sin") && same_trig_arg(a, b) {
        return Some(Term::int(1));
    }
    None
}

fn is_trig_sq(expr: &Term, name: &str) -> bool {
    match expr {
        Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 => {
            matches!(number_from_term(&args[1]), Some(n) if *n == Number::small_int(2))
                && matches!(&args[0], Term::Application { head: h, arguments: a } if h.is_symbol(name) && a.len() == 1)
        }
        _ => false,
    }
}

fn same_trig_arg(a: &Term, b: &Term) -> bool {
    fn arg(expr: &Term) -> Option<&Term> {
        match expr {
            Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 => match &args[0] {
                Term::Application { arguments: inner, .. } if inner.len() == 1 => Some(&inner[0]),
                _ => None,
            },
            _ => None,
        }
    }
    match (arg(a), arg(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

fn eval_replace_all(expr: &Term, rules: &Term, depth: u32) -> Term {
    let rule_list: Vec<(Term, Term)> = match rules {
        Term::List(items) => items.iter().filter_map(rule_pair).collect(),
        other => rule_pair(other).into_iter().collect(),
    };
    let mut cur = clone_term(expr);
    for (lhs, rhs) in rule_list {
        cur = replace_literal(&cur, &lhs, &rhs);
    }
    evaluate_depth(&cur, depth + 1)
}

fn rule_pair(expr: &Term) -> Option<(Term, Term)> {
    match expr {
        Term::Application { head, arguments: args }
            if args.len() == 2 && (head.is_symbol("Rule") || head.is_symbol("RuleDelayed")) =>
        {
            Some((clone_term(&args[0]), clone_term(&args[1])))
        }
        _ => None,
    }
}

fn replace_literal(expr: &Term, lhs: &Term, rhs: &Term) -> Term {
    if expr == lhs {
        return clone_term(rhs);
    }
    match expr {
        Term::List(items) => Term::List(items.iter().map(|i| replace_literal(i, lhs, rhs)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(replace_literal(head, lhs, rhs)),
            arguments: args.iter().map(|a| replace_literal(a, lhs, rhs)).collect(),
        },
        other => clone_term(other),
    }
}

fn eval_part(expr: &Term, index: &Term) -> Term {
    eval_part_outcome(expr, index).term
}

fn eval_part_outcome(expr: &Term, index: &Term) -> EvalOutcome {
    let idx = match number_from_term(index).and_then(|n| n.as_exact_integer()) {
        Some(v) => v,
        None => {
            return EvalOutcome::unevaluated(Term::apply("Part", vec![clone_term(expr), clone_term(index)]));
        }
    };
    match expr {
        Term::List(items) => {
            let len = items.len() as u64;
            if idx == 0 {
                // Mathematica: Part[list, 0] is the head `List`.
                return EvalOutcome::value(Term::symbol("List"));
            }
            let i = if idx > 0 {
                (idx - 1) as usize
            } else {
                // negative: from end
                let n = items.len() as i64;
                let pos = n + idx;
                if pos < 0 || pos as usize >= items.len() {
                    let term = Term::apply("Part", vec![clone_term(expr), clone_term(index)]);
                    return EvalOutcome::invalid(term, invalid_index_diagnostic(idx, Some(len)));
                }
                pos as usize
            };
            match items.get(i) {
                Some(item) => EvalOutcome::value(clone_term(item)),
                None => {
                    let term = Term::apply("Part", vec![clone_term(expr), clone_term(index)]);
                    EvalOutcome::invalid(term, invalid_index_diagnostic(idx, Some(len)))
                }
            }
        }
        _ => EvalOutcome::unevaluated(Term::apply("Part", vec![clone_term(expr), clone_term(index)])),
    }
}

fn substitute_slot(body: &Term, value: &Term) -> Term {
    match body {
        Term::Atom(Atom::Symbol(s)) if s == "#" || s == "#1" => clone_term(value),
        Term::Atom(_) => clone_term(body),
        Term::List(items) => Term::List(items.iter().map(|i| substitute_slot(i, value)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(substitute_slot(head, value)),
            arguments: args.iter().map(|a| substitute_slot(a, value)).collect(),
        },
    }
}
