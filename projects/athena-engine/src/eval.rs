//! 引擎桥接 [`Term`] 上的内建求值。

use std::cmp::Ordering;

use athena_numeric::{
    Integer, Number, Rational, abs as num_abs, add as num_add, compare as num_compare, div as num_div,
    factorial as num_factorial, mul as num_mul, pow as num_pow, sqrt as num_sqrt, to_f64_lossy as num_to_f64_lossy,
};
use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, Result, Severity};

use crate::{
    linear_algebra::{MatrixValue, SolveDisposition, det_bareiss, matmul, solve_exact},
    numeric_clone::{clone_number, clone_rational, clone_term, clone_terms},
    term::{Atom, Term, number_from_term},
};

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
        Self { term, kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics: Vec::new() }
    }

    /// 未求值保留。
    pub fn unevaluated(term: Term) -> Self {
        Self { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Unknown, diagnostics: Vec::new() }
    }

    /// 硬失败：带 Error 诊断，状态为 [`ComputationStatus::Invalid`]。
    pub fn invalid(term: Term, diagnostic: Diagnostic) -> Self {
        Self { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics: vec![diagnostic] }
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
    Diagnostic::new(DiagnosticCode::NonBooleanCondition).detail("expected", "Boolean").detail("got", got)
}

/// 将项解释为 typed Boolean。优先 [`Atom::Boolean`]；兼容符号 `True`/`False` 与精确 `1`/`0`。其它返回 `None`。
pub fn as_boolean(expr: &Term) -> Option<bool> {
    match expr {
        Term::Atom(Atom::Boolean(b)) => Some(*b),
        Term::Atom(Atom::Symbol(s)) if s == "True" => Some(true),
        Term::Atom(Atom::Symbol(s)) if s == "False" => Some(false),
        other => number_from_term(other).and_then(|n| {
            if n.is_zero() {
                Some(false)
            }
            else if *n == Number::small_int(1) {
                Some(true)
            }
            else {
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

/// Session 级符号定义（Living 25：仍为 `Term` 桥，非 AthenaIR 身份）。
#[derive(Debug, PartialEq)]
pub enum Definition {
    /// `Set`：赋值时求值，查表直接替换。
    Own(Term),
    /// `SetDelayed`：存未求值 RHS，使用时再求值。
    Delayed(Term),
    /// Patterned down-values：`f[x_] := rhs` keyed by head `f`.
    DownValues(Vec<(Term, Term)>),
}

/// Own / Delayed / DownValues 符号定义表。
pub type DefinitionMap = std::collections::HashMap<String, Definition>;

fn definition_term(def: &Definition) -> Option<Term> {
    match def {
        Definition::Own(t) | Definition::Delayed(t) => Some(clone_term(t)),
        Definition::DownValues(_) => None,
    }
}

/// 带持久定义求值：顶层 `Set` / `SetDelayed` / `CompoundExpression` 会回写 `definitions`。
///
/// 无状态 [`evaluate`] 不触碰此表。局部 `With`/`Module`/`Block` 仍不泄漏到 `definitions`。
pub fn evaluate_with_definitions(definitions: &mut DefinitionMap, expr: &Term) -> EvalOutcome {
    // Scoping forms must see outer defs without rewriting localized bodies first.
    if let Term::Application { head, arguments: args } = expr {
        if let Term::Atom(Atom::Symbol(name)) = head.as_ref() {
            match name.as_str() {
                "With" | "Module" | "Block" => return eval_local_scope(name, args, 0, definitions),
                "CompoundExpression" => return eval_compound_into(args, definitions, 0),
                _ => {}
            }
        }
    }

    let rewritten = apply_bindings(expr, definitions);

    if let Some((name, rhs)) = match_set(&rewritten) {
        let o = evaluate_depth_outcome(&rhs, 0);
        if !o.has_error() {
            definitions.insert(name, Definition::Own(clone_term(&o.term)));
        }
        return o;
    }

    if let Some((name, lhs, rhs)) = match_set_delayed_down(&rewritten) {
        insert_down_value(definitions, name, lhs, rhs);
        return EvalOutcome::value(Term::null());
    }

    if let Some((name, rhs)) = match_set_delayed(&rewritten) {
        definitions.insert(name, Definition::Delayed(clone_term(&rhs)));
        return EvalOutcome::value(Term::null());
    }

    evaluate_depth_outcome(&rewritten, 0)
}

/// [`evaluate_with_definitions`] 的项出口。
pub fn evaluate_in(definitions: &mut DefinitionMap, expr: &Term) -> Term {
    evaluate_with_definitions(definitions, expr).term
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
        Term::Atom(Atom::Symbol(s)) if s == "True" => EvalOutcome::value(Term::boolean(true)),
        Term::Atom(Atom::Symbol(s)) if s == "False" => EvalOutcome::value(Term::boolean(false)),
        Term::Atom(Atom::Symbol(s)) if s == "Null" => EvalOutcome::value(Term::null()),
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
                EvalOutcome { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics }
            }
            else if all_value {
                EvalOutcome { term, kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
            }
            else {
                EvalOutcome { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Partial, diagnostics }
            }
        }
        Term::Application { head, arguments: args } => {
            let head_o = evaluate_depth_outcome(head, depth + 1);
            let mut diagnostics = head_o.diagnostics;

            // HoldAll / 短路形式：不得先求值全部参数。
            if let Some(mut out) = eval_special_form(&head_o.term, args, depth) {
                if !diagnostics.is_empty() {
                    diagnostics.append(&mut out.diagnostics);
                    out.diagnostics = diagnostics;
                    if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                        out.status = ComputationStatus::Invalid;
                        out.kind = EvalKind::Unevaluated;
                    }
                }
                return out;
            }

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

/// HoldAll / 条件短路：在通用「先求值参数」路径之前处理。
fn eval_special_form(head: &Term, args: &[Term], depth: u32) -> Option<EvalOutcome> {
    let name = match head {
        Term::Atom(Atom::Symbol(s)) => s.as_str(),
        _ => return None,
    };
    match name {
        "Hold" | "HoldForm" => {
            Some(EvalOutcome::unevaluated(Term::Application { head: Box::new(clone_term(head)), arguments: clone_terms(args) }))
        }
        "If" => Some(eval_if(args, depth)),
        "Which" => Some(eval_which(args, depth)),
        "While" => Some(eval_while(args, depth, &mut DefinitionMap::new())),
        "For" => Some(eval_for(args, depth, &mut DefinitionMap::new())),
        "CompoundExpression" => Some(eval_compound(args, depth)),
        "With" | "Module" | "Block" => Some(eval_local_scope(name, args, depth, &DefinitionMap::new())),
        "MatchQ" => Some(eval_match_q(args, depth)),
        "Cases" => Some(eval_cases(args, depth)),
        "Table" => Some(eval_table(args, depth)),
        "Sum" => Some(eval_sum_dispatch(args, depth)),
        "Product" => Some(eval_sum_product("Product", args, depth)),
        // Chained comparisons must see unevaluated nested compare ops (`1 < 2 < 3`).
        "Less" | "Greater" | "LessEqual" | "GreaterEqual" => Some(eval_compare_chain(name, args, depth)),
        "Try" => Some(eval_try(args, depth)),
        "Blank" | "BlankSequence" | "BlankNullSequence" | "Pattern" => {
            Some(EvalOutcome::unevaluated(Term::Application { head: Box::new(clone_term(head)), arguments: clone_terms(args) }))
        }
        _ => None,
    }
}

fn eval_match_q(args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply("MatchQ", clone_terms(args)));
    }
    let mut expr_o = evaluate_depth_outcome(&args[0], depth + 1);
    // Pattern argument is Hold-ish: do not evaluate Blank/Pattern away.
    let matched = pattern_matches(&expr_o.term, &args[1]);
    expr_o.term = Term::boolean(matched);
    expr_o.kind = EvalKind::Value;
    expr_o.status = ComputationStatus::Exact;
    expr_o
}

fn eval_cases(args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply("Cases", clone_terms(args)));
    }
    let mut list_o = evaluate_depth_outcome(&args[0], depth + 1);
    let Term::List(items) = &list_o.term
    else {
        return EvalOutcome::unevaluated(Term::apply("Cases", vec![list_o.term, clone_term(&args[1])]));
    };
    let pat = &args[1];
    let out: Vec<Term> = items.iter().filter(|item| pattern_matches(item, pat)).map(clone_term).collect();
    list_o.term = Term::List(out);
    list_o.kind = EvalKind::Value;
    list_o.status = ComputationStatus::Exact;
    list_o
}

/// Minimal pattern matcher for Feature Gap: `Blank` / typed `Blank[h]` / `Pattern[name, p]` / literal.
fn pattern_matches(expr: &Term, pattern: &Term) -> bool {
    match pattern {
        Term::Application { head, arguments: args } if head.is_symbol("Blank") => match args.as_slice() {
            [] => true,
            [head_pat] => expr_has_head(expr, head_pat),
            _ => false,
        },
        Term::Application { head, arguments: args } if head.is_symbol("Pattern") && args.len() == 2 => {
            pattern_matches(expr, &args[1])
        }
        other => terms_structurally_equal(expr, other),
    }
}

fn expr_has_head(expr: &Term, head_pat: &Term) -> bool {
    match head_pat {
        Term::Atom(Atom::Symbol(name)) => match name.as_str() {
            "Integer" => number_from_term(expr).is_some_and(|n| n.as_exact_integer().is_some() || n.as_integer().is_some()),
            "Symbol" => matches!(expr, Term::Atom(Atom::Symbol(_))),
            "List" => matches!(expr, Term::List(_)),
            "String" => matches!(expr, Term::Atom(Atom::String(_))),
            other => expr.head_name() == Some(other),
        },
        _ => false,
    }
}

fn terms_structurally_equal(a: &Term, b: &Term) -> bool {
    match (a, b) {
        (Term::Atom(Atom::Symbol(x)), Term::Atom(Atom::Symbol(y))) => x == y,
        (Term::Atom(Atom::Boolean(x)), Term::Atom(Atom::Boolean(y))) => x == y,
        (Term::Atom(Atom::Null), Term::Atom(Atom::Null)) => true,
        (Term::Atom(Atom::String(x)), Term::Atom(Atom::String(y))) => x == y,
        (Term::Atom(Atom::Number(x)), Term::Atom(Atom::Number(y))) => x == y,
        (Term::List(xs), Term::List(ys)) if xs.len() == ys.len() => {
            xs.iter().zip(ys.iter()).all(|(l, r)| terms_structurally_equal(l, r))
        }
        (Term::Application { head: hx, arguments: ax }, Term::Application { head: hy, arguments: ay })
            if ax.len() == ay.len() =>
        {
            terms_structurally_equal(hx, hy) && ax.iter().zip(ay.iter()).all(|(l, r)| terms_structurally_equal(l, r))
        }
        _ => false,
    }
}

fn eval_if(args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() < 2 || args.len() > 4 {
        return EvalOutcome::unevaluated(Term::apply("If", clone_terms(args)));
    }

    let cond_o = evaluate_depth_outcome(&args[0], depth + 1);
    let mut diagnostics = cond_o.diagnostics;
    match as_boolean(&cond_o.term) {
        Some(true) => {
            let mut out = evaluate_depth_outcome(&args[1], depth + 1);
            diagnostics.append(&mut out.diagnostics);
            out.diagnostics = diagnostics;
            if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                out.status = ComputationStatus::Invalid;
                out.kind = EvalKind::Unevaluated;
            }
            out
        }
        Some(false) => {
            if args.len() >= 3 {
                let mut out = evaluate_depth_outcome(&args[2], depth + 1);
                diagnostics.append(&mut out.diagnostics);
                out.diagnostics = diagnostics;
                if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                    out.status = ComputationStatus::Invalid;
                    out.kind = EvalKind::Unevaluated;
                }
                out
            }
            else {
                EvalOutcome { term: Term::null(), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
            }
        }
        None => {
            if args.len() == 4 {
                let mut out = evaluate_depth_outcome(&args[3], depth + 1);
                diagnostics.append(&mut out.diagnostics);
                out.diagnostics = diagnostics;
                if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                    out.status = ComputationStatus::Invalid;
                    out.kind = EvalKind::Unevaluated;
                }
                out
            }
            else {
                let summary = term_summary(&cond_o.term);
                let mut held = vec![cond_o.term];
                held.extend(clone_terms(&args[1..]));
                let term = Term::apply("If", held);
                diagnostics.push(non_boolean_condition_diagnostic(&summary));
                EvalOutcome { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics }
            }
        }
    }
}

fn eval_which(args: &[Term], depth: u32) -> EvalOutcome {
    if args.is_empty() || args.len() % 2 != 0 {
        return EvalOutcome::unevaluated(Term::apply("Which", clone_terms(args)));
    }

    let mut diagnostics = Vec::new();
    let mut uneval_pairs: Vec<Term> = Vec::new();
    let mut i = 0;
    while i + 1 < args.len() {
        let cond_o = evaluate_depth_outcome(&args[i], depth + 1);
        diagnostics.extend(cond_o.diagnostics);
        match as_boolean(&cond_o.term) {
            Some(true) => {
                let mut out = evaluate_depth_outcome(&args[i + 1], depth + 1);
                diagnostics.append(&mut out.diagnostics);
                out.diagnostics = diagnostics;
                if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
                    out.status = ComputationStatus::Invalid;
                    out.kind = EvalKind::Unevaluated;
                }
                return out;
            }
            Some(false) => {
                // skip branch
            }
            None => {
                uneval_pairs.push(cond_o.term);
                uneval_pairs.push(clone_term(&args[i + 1]));
            }
        }
        i += 2;
    }

    if uneval_pairs.is_empty() {
        EvalOutcome { term: Term::null(), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
    }
    else {
        let summary = term_summary(&uneval_pairs[0]);
        diagnostics.push(non_boolean_condition_diagnostic(&summary));
        EvalOutcome {
            term: Term::apply("Which", uneval_pairs),
            kind: EvalKind::Unevaluated,
            status: ComputationStatus::Invalid,
            diagnostics,
        }
    }
}

fn term_summary(term: &Term) -> String {
    match term {
        Term::Atom(Atom::Symbol(s)) => s.clone(),
        Term::Atom(Atom::String(_)) => "String".into(),
        Term::Atom(Atom::Number(_)) => "Number".into(),
        Term::Atom(Atom::Boolean(true)) => "True".into(),
        Term::Atom(Atom::Boolean(false)) => "False".into(),
        Term::Atom(Atom::Null) => "Null".into(),
        Term::List(_) => "List".into(),
        Term::Application { head, .. } => head.head_name().unwrap_or("Application").to_string(),
    }
}

/// `While[cond, body]` — false condition skips body (`while 0` → `Null`).
///
/// Body `Set` writes into `env` so compound `s=0; while …` accumulators work.
fn eval_while(args: &[Term], depth: u32, env: &mut DefinitionMap) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply("While", clone_terms(args)));
    }
    let mut diagnostics = Vec::new();
    let mut last = Term::null();
    let mut ran = false;
    for _ in 0..1024u32 {
        let cond = apply_bindings(&args[0], env);
        let cond_o = evaluate_depth_outcome(&cond, depth + 1);
        diagnostics.extend(cond_o.diagnostics);
        match as_boolean(&cond_o.term) {
            Some(false) => {
                return EvalOutcome {
                    term: if ran { last } else { Term::null() },
                    kind: EvalKind::Value,
                    status: ComputationStatus::Exact,
                    diagnostics,
                };
            }
            Some(true) => {
                ran = true;
                let mut body_o = eval_stmt_into(&args[1], env, depth + 1);
                diagnostics.append(&mut body_o.diagnostics);
                last = body_o.term;
                if diagnostics.iter().any(|d| d.severity == Severity::Error) {
                    return EvalOutcome {
                        term: last,
                        kind: EvalKind::Unevaluated,
                        status: ComputationStatus::Invalid,
                        diagnostics,
                    };
                }
            }
            None => {
                diagnostics.push(non_boolean_condition_diagnostic(&term_summary(&cond_o.term)));
                let term = Term::apply("While", vec![cond_o.term, clone_term(&args[1])]);
                return EvalOutcome { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics };
            }
        }
    }
    let term = Term::apply("While", clone_terms(args));
    diagnostics.push(unsupported_operation("While"));
    EvalOutcome { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics }
}

/// `Table[body, {i, n}]` / `{i, a, b}` / `{i, a, b, step}` / `{n}` — body HoldAll-ish.
fn eval_table(args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply("Table", clone_terms(args)));
    }
    let iter_o = evaluate_depth_outcome(&args[1], depth + 1);
    let mut diagnostics = iter_o.diagnostics;
    let Some((var, values)) = expand_iterator(&iter_o.term)
    else {
        return EvalOutcome::unevaluated(Term::apply("Table", vec![clone_term(&args[0]), iter_o.term]));
    };
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let body = match &var {
            Some(name) => substitute_symbol(&args[0], name, &value),
            None => clone_term(&args[0]),
        };
        let mut body_o = evaluate_depth_outcome(&body, depth + 1);
        diagnostics.append(&mut body_o.diagnostics);
        out.push(body_o.term);
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            return EvalOutcome {
                term: Term::List(out),
                kind: EvalKind::Unevaluated,
                status: ComputationStatus::Invalid,
                diagnostics,
            };
        }
    }
    EvalOutcome { term: Term::List(out), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
}

/// `Sum[body, iterator]` / `Product[body, iterator]` — fold evaluated Table values.
/// `Sum[list]` → 数组求和；`Sum[body, iterator]` → 符号求和折叠。
fn eval_sum_dispatch(args: &[Term], depth: u32) -> EvalOutcome {
    match args {
        [only] => {
            let mut o = evaluate_depth_outcome(only, depth + 1);
            let summed = eval_array_sum(&o.term);
            if summed.has_error() {
                o.diagnostics.extend(summed.diagnostics);
                o.term = summed.term;
                o.kind = summed.kind;
                o.status = summed.status;
                return o;
            }
            o.diagnostics.extend(summed.diagnostics);
            o.term = summed.term;
            o.kind = summed.kind;
            o.status = summed.status;
            o
        }
        [_, _] => eval_sum_product("Sum", args, depth),
        _ => EvalOutcome::unevaluated(Term::apply("Sum", clone_terms(args))),
    }
}

fn eval_sum_product(head: &str, args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply(head, clone_terms(args)));
    }
    let mut table_o = eval_table(args, depth);
    let Term::List(items) = table_o.term
    else {
        return EvalOutcome::unevaluated(Term::apply(head, vec![clone_term(&args[0]), table_o.term]));
    };
    if table_o.kind != EvalKind::Value {
        return EvalOutcome {
            term: Term::apply(head, vec![clone_term(&args[0]), Term::List(items)]),
            kind: table_o.kind,
            status: table_o.status,
            diagnostics: table_o.diagnostics,
        };
    }
    let folded = match head {
        "Sum" => {
            if items.is_empty() {
                Term::int(0)
            }
            else {
                eval_plus(items)
            }
        }
        "Product" => {
            if items.is_empty() {
                Term::int(1)
            }
            else {
                eval_times(items)
            }
        }
        _ => Term::apply(head, clone_terms(args)),
    };
    table_o.term = folded;
    table_o.kind = EvalKind::Value;
    table_o.status = ComputationStatus::Exact;
    table_o
}

/// Expand `{i,n}` / `{i,a,b}` / `{i,a,b,step}` / `{n}` into optional binder + values.
fn expand_iterator(spec: &Term) -> Option<(Option<String>, Vec<Term>)> {
    let Term::List(items) = spec
    else {
        return None;
    };
    match items.as_slice() {
        [Term::Atom(Atom::Symbol(var)), n] => {
            let n = number_from_term(n)?.as_exact_integer()?;
            Some((Some(var.clone()), range_ints(1, n, 1)?))
        }
        [Term::Atom(Atom::Symbol(var)), a, b] => {
            let a = number_from_term(a)?.as_exact_integer()?;
            let b = number_from_term(b)?.as_exact_integer()?;
            Some((Some(var.clone()), range_ints(a, b, 1)?))
        }
        [Term::Atom(Atom::Symbol(var)), a, b, step] => {
            let a = number_from_term(a)?.as_exact_integer()?;
            let b = number_from_term(b)?.as_exact_integer()?;
            let step = number_from_term(step)?.as_exact_integer()?;
            Some((Some(var.clone()), range_ints(a, b, step)?))
        }
        [n] => {
            let n = number_from_term(n)?.as_exact_integer()?;
            Some((None, range_ints(1, n, 1)?))
        }
        _ => None,
    }
}

fn range_ints(a: i64, b: i64, step: i64) -> Option<Vec<Term>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut x = a;
    if step > 0 {
        while x <= b {
            out.push(Term::int(x));
            x += step;
        }
    }
    else {
        while x >= b {
            out.push(Term::int(x));
            x += step;
        }
    }
    Some(out)
}

/// `For[var, iterator, body]` — iterator is a list (often from `Span`).
///
/// Body `Set` writes into `env` so `s=0; for i=1:3, s=s+i; end; s` accumulates.
fn eval_for(args: &[Term], depth: u32, env: &mut DefinitionMap) -> EvalOutcome {
    if args.len() != 3 {
        return EvalOutcome::unevaluated(Term::apply("For", clone_terms(args)));
    }
    let var = match &args[0] {
        Term::Atom(Atom::Symbol(s)) => s.as_str(),
        _ => {
            return EvalOutcome::unevaluated(Term::apply("For", clone_terms(args)));
        }
    };
    let iter = apply_bindings(&args[1], env);
    let iter_o = evaluate_depth_outcome(&iter, depth + 1);
    let mut diagnostics = iter_o.diagnostics;
    let values = match iter_o.term {
        Term::List(items) => items,
        other => {
            let term = Term::apply("For", vec![clone_term(&args[0]), other, clone_term(&args[2])]);
            return EvalOutcome::unevaluated(term);
        }
    };
    let mut last = Term::null();
    for value in values {
        let body = substitute_symbol(&args[2], var, &value);
        let mut body_o = eval_stmt_into(&body, env, depth + 1);
        diagnostics.append(&mut body_o.diagnostics);
        last = body_o.term;
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            return EvalOutcome { term: last, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics };
        }
    }
    EvalOutcome { term: last, kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
}

/// Evaluate one statement into `env` (`Set` / nested compound / For / While).
fn eval_stmt_into(stmt: &Term, env: &mut DefinitionMap, depth: u32) -> EvalOutcome {
    // Control forms must keep unevaluated bodies so loop accumulators see fresh bindings.
    if let Term::Application { head, arguments: args } = stmt {
        if head.is_symbol("CompoundExpression") {
            return eval_compound_into(args, env, depth + 1);
        }
        if head.is_symbol("For") {
            return eval_for(args, depth + 1, env);
        }
        if head.is_symbol("While") {
            return eval_while(args, depth + 1, env);
        }
        if head.is_symbol("Try") {
            return eval_try(args, depth + 1);
        }
    }
    let rewritten = apply_bindings(stmt, env);
    if let Some((name, rhs)) = match_set(&rewritten) {
        let mut o = evaluate_depth_outcome(&rhs, depth + 1);
        if !o.has_error() {
            env.insert(name, Definition::Own(clone_term(&o.term)));
        }
        return o;
    }
    if let Some((name, lhs, rhs)) = match_set_delayed_down(&rewritten) {
        insert_down_value(env, name, lhs, rhs);
        return EvalOutcome::value(Term::null());
    }
    if let Some((name, rhs)) = match_set_delayed(&rewritten) {
        env.insert(name, Definition::Delayed(clone_term(&rhs)));
        return EvalOutcome::value(Term::null());
    }
    evaluate_depth_outcome(&rewritten, depth + 1)
}

/// Expand left-associative compare chains: `Less[Less[1,2],3]` → `And[1<2, 2<3]`.
fn eval_compare_chain(op: &str, args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply(op, clone_terms(args)));
    }
    if let Term::Application { head, arguments: inner } = &args[0] {
        if is_compare_head(head) && inner.len() == 2 {
            let left_o = evaluate_depth_outcome(&args[0], depth + 1);
            let mid = &inner[1];
            let right_term = Term::apply(op, vec![clone_term(mid), clone_term(&args[1])]);
            let right_o = evaluate_depth_outcome(&right_term, depth + 1);
            let mut diagnostics = left_o.diagnostics;
            diagnostics.extend(right_o.diagnostics);
            match (as_boolean(&left_o.term), as_boolean(&right_o.term)) {
                (Some(a), Some(b)) => {
                    return EvalOutcome {
                        term: Term::boolean(a && b),
                        kind: EvalKind::Value,
                        status: ComputationStatus::Exact,
                        diagnostics,
                    };
                }
                _ => {
                    let term = Term::apply("And", vec![left_o.term, right_o.term]);
                    return EvalOutcome { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Partial, diagnostics };
                }
            }
        }
    }
    let left_o = evaluate_depth_outcome(&args[0], depth + 1);
    let right_o = evaluate_depth_outcome(&args[1], depth + 1);
    let mut diagnostics = left_o.diagnostics;
    diagnostics.extend(right_o.diagnostics);
    let term = match op {
        "Less" => eval_compare("Less", &left_o.term, &right_o.term, |o| o == Ordering::Less),
        "Greater" => eval_compare("Greater", &left_o.term, &right_o.term, |o| o == Ordering::Greater),
        "LessEqual" => eval_compare("LessEqual", &left_o.term, &right_o.term, |o| o != Ordering::Greater),
        "GreaterEqual" => eval_compare("GreaterEqual", &left_o.term, &right_o.term, |o| o != Ordering::Less),
        _ => Term::apply(op, vec![left_o.term, right_o.term]),
    };
    let mut out = wrap_compare(term, op);
    if !diagnostics.is_empty() {
        diagnostics.append(&mut out.diagnostics);
        out.diagnostics = diagnostics;
    }
    out
}

fn is_compare_head(head: &Term) -> bool {
    matches!(
        head.head_name(),
        Some("Less") | Some("Greater") | Some("LessEqual") | Some("GreaterEqual") | Some("Equal") | Some("Unequal")
    )
}

/// `Try[body, catch]` — on Error diagnostics evaluate catch, else body value.
fn eval_try(args: &[Term], depth: u32) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply("Try", clone_terms(args)));
    }
    let body_o = evaluate_depth_outcome(&args[0], depth + 1);
    if body_o.has_error() {
        return evaluate_depth_outcome(&args[1], depth + 1);
    }
    body_o
}

fn substitute_symbol(expr: &Term, name: &str, value: &Term) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) if s == name => clone_term(value),
        Term::Atom(_) => clone_term(expr),
        Term::List(items) => Term::List(items.iter().map(|i| substitute_symbol(i, name, value)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(substitute_symbol(head, name, value)),
            arguments: args.iter().map(|a| substitute_symbol(a, name, value)).collect(),
        },
    }
}

/// Sequential statements with temporary `Set` / `SetDelayed` bindings (`x=5; x+1` → `6`).
fn eval_compound(args: &[Term], depth: u32) -> EvalOutcome {
    let mut env = DefinitionMap::new();
    eval_compound_into(args, &mut env, depth)
}

/// Like [`eval_compound`], but reads/writes an existing definition map (Session persistence).
fn eval_compound_into(args: &[Term], env: &mut DefinitionMap, depth: u32) -> EvalOutcome {
    if args.is_empty() {
        return EvalOutcome::value(Term::null());
    }

    let mut diagnostics = Vec::new();
    let mut last = Term::null();

    for arg in args {
        // Do not `apply_bindings` into For/While bodies before the loop runs.
        if let Term::Application { head, .. } = arg {
            if head.is_symbol("For") || head.is_symbol("While") || head.is_symbol("Try") || head.is_symbol("CompoundExpression")
            {
                let mut o = eval_stmt_into(arg, env, depth + 1);
                diagnostics.append(&mut o.diagnostics);
                last = o.term;
                if diagnostics.iter().any(|d| d.severity == Severity::Error) {
                    return EvalOutcome {
                        term: last,
                        kind: EvalKind::Unevaluated,
                        status: ComputationStatus::Invalid,
                        diagnostics,
                    };
                }
                continue;
            }
        }

        let rewritten = apply_bindings(arg, env);
        if let Some((name, rhs)) = match_set(&rewritten) {
            let mut o = evaluate_depth_outcome(&rhs, depth + 1);
            diagnostics.append(&mut o.diagnostics);
            env.insert(name, Definition::Own(clone_term(&o.term)));
            last = o.term;
        }
        else if let Some((name, lhs, rhs)) = match_set_delayed_down(&rewritten) {
            insert_down_value(env, name, lhs, rhs);
            last = Term::null();
        }
        else if let Some((name, rhs)) = match_set_delayed(&rewritten) {
            env.insert(name, Definition::Delayed(clone_term(&rhs)));
            last = Term::null();
        }
        else {
            let mut o = evaluate_depth_outcome(&rewritten, depth + 1);
            diagnostics.append(&mut o.diagnostics);
            last = o.term;
        }
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            return EvalOutcome { term: last, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics };
        }
    }

    EvalOutcome { term: last, kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
}

fn match_set(term: &Term) -> Option<(String, Term)> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Set") && args.len() == 2 => match &args[0] {
            Term::Atom(Atom::Symbol(s)) => Some((s.clone(), clone_term(&args[1]))),
            _ => None,
        },
        _ => None,
    }
}

fn match_set_delayed(term: &Term) -> Option<(String, Term)> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("SetDelayed") && args.len() == 2 => match &args[0] {
            Term::Atom(Atom::Symbol(s)) => Some((s.clone(), clone_term(&args[1]))),
            _ => None,
        },
        _ => None,
    }
}

/// `f[x_] := rhs` → `(f, lhs, rhs)`.
fn match_set_delayed_down(term: &Term) -> Option<(String, Term, Term)> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("SetDelayed") && args.len() == 2 => match &args[0] {
            Term::Application { head: lhs_head, .. } => match lhs_head.as_ref() {
                Term::Atom(Atom::Symbol(name)) => Some((name.clone(), clone_term(&args[0]), clone_term(&args[1]))),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn insert_down_value(env: &mut DefinitionMap, name: String, lhs: Term, rhs: Term) {
    match env.get_mut(&name) {
        Some(Definition::DownValues(rules)) => rules.push((lhs, rhs)),
        Some(other) => *other = Definition::DownValues(vec![(lhs, rhs)]),
        None => {
            env.insert(name, Definition::DownValues(vec![(lhs, rhs)]));
        }
    }
}

/// `Module` 局部：`x=1` 或裸 `x`。
fn match_module_local(term: &Term) -> Option<(String, Option<Term>)> {
    if let Some((name, rhs)) = match_set(term) {
        return Some((name, Some(rhs)));
    }
    match term {
        Term::Atom(Atom::Symbol(s)) => Some((s.clone(), None)),
        _ => None,
    }
}

fn next_module_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn apply_symbol_rename(expr: &Term, rename: &std::collections::HashMap<String, String>) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) => {
            if let Some(u) = rename.get(s) {
                Term::symbol(u)
            }
            else {
                clone_term(expr)
            }
        }
        Term::Atom(_) => clone_term(expr),
        Term::List(items) => Term::List(items.iter().map(|i| apply_symbol_rename(i, rename)).collect()),
        Term::Application { head, arguments: args } => {
            if (head.is_symbol("Set") || head.is_symbol("SetDelayed")) && args.len() == 2 {
                Term::Application {
                    head: Box::new(clone_term(head)),
                    arguments: vec![clone_term(&args[0]), apply_symbol_rename(&args[1], rename)],
                }
            }
            else {
                Term::Application {
                    head: Box::new(apply_symbol_rename(head, rename)),
                    arguments: args.iter().map(|a| apply_symbol_rename(a, rename)).collect(),
                }
            }
        }
    }
}

/// `With` / `Module` / `Block` — local bindings from `{x=1,…}` then evaluate body.
///
/// Living 25: still on legacy `Term` bridge for Feature Gap.
/// `Module` 对局部符号做 `$n` 唯一化重命名；`With`/`Block` 仍为直接词法替换。
/// `outer` 为 Session / 外层定义（局部同名遮蔽，不写回）。
fn eval_local_scope(head: &str, args: &[Term], depth: u32, outer: &DefinitionMap) -> EvalOutcome {
    if args.len() != 2 {
        return EvalOutcome::unevaluated(Term::apply(head, clone_terms(args)));
    }

    let locals = match &args[0] {
        Term::List(items) => items.as_slice(),
        other => {
            return EvalOutcome::unevaluated(Term::apply(head, vec![clone_term(other), clone_term(&args[1])]));
        }
    };

    if head == "Module" {
        return eval_module(locals, &args[1], depth, outer);
    }

    let mut env = clone_definition_map(outer);
    let mut diagnostics = Vec::new();
    for item in locals {
        let Some((name, rhs)) = match_set(item)
        else {
            return EvalOutcome::unevaluated(Term::apply(head, clone_terms(args)));
        };
        let rewritten_rhs = apply_bindings(&rhs, &env);
        let mut o = evaluate_depth_outcome(&rewritten_rhs, depth + 1);
        diagnostics.append(&mut o.diagnostics);
        if diagnostics.iter().any(|d| d.severity == Severity::Error) {
            return EvalOutcome {
                term: Term::apply(head, clone_terms(args)),
                kind: EvalKind::Unevaluated,
                status: ComputationStatus::Invalid,
                diagnostics,
            };
        }
        env.insert(name, Definition::Own(o.term));
    }

    let body = apply_bindings(&args[1], &env);
    let mut out = evaluate_depth_outcome(&body, depth + 1);
    diagnostics.append(&mut out.diagnostics);
    out.diagnostics = diagnostics;
    if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
        out.status = ComputationStatus::Invalid;
        out.kind = EvalKind::Unevaluated;
    }
    out
}

fn clone_definition_map(env: &DefinitionMap) -> DefinitionMap {
    env.iter()
        .map(|(k, v)| {
            (
                k.clone(),
                match v {
                    Definition::Own(t) => Definition::Own(clone_term(t)),
                    Definition::Delayed(t) => Definition::Delayed(clone_term(t)),
                    Definition::DownValues(rules) => {
                        Definition::DownValues(rules.iter().map(|(lhs, rhs)| (clone_term(lhs), clone_term(rhs))).collect())
                    }
                },
            )
        })
        .collect()
}

fn eval_module(locals: &[Term], body: &Term, depth: u32, outer: &DefinitionMap) -> EvalOutcome {
    let mut rename = std::collections::HashMap::new();
    let mut init_env = clone_definition_map(outer);
    let mut diagnostics = Vec::new();
    let mut uniq_values: std::collections::HashMap<String, Term> = std::collections::HashMap::new();

    for item in locals {
        let Some((name, rhs_opt)) = match_module_local(item)
        else {
            return EvalOutcome::unevaluated(Term::apply("Module", vec![Term::List(clone_terms(locals)), clone_term(body)]));
        };
        let uniq = format!("{}${}", name, next_module_id());
        rename.insert(name.clone(), uniq.clone());
        if let Some(rhs) = rhs_opt {
            let rewritten_rhs = apply_bindings(&rhs, &init_env);
            let mut o = evaluate_depth_outcome(&rewritten_rhs, depth + 1);
            diagnostics.append(&mut o.diagnostics);
            if diagnostics.iter().any(|d| d.severity == Severity::Error) {
                return EvalOutcome {
                    term: Term::apply("Module", vec![Term::List(clone_terms(locals)), clone_term(body)]),
                    kind: EvalKind::Unevaluated,
                    status: ComputationStatus::Invalid,
                    diagnostics,
                };
            }
            // Sequential Module inits see prior locals under the original name.
            init_env.insert(name.clone(), Definition::Own(clone_term(&o.term)));
            uniq_values.insert(uniq, o.term);
        }
    }

    let mut body_env = clone_definition_map(outer);
    for orig in rename.keys() {
        body_env.remove(orig);
    }
    for (uniq, val) in uniq_values {
        body_env.insert(uniq, Definition::Own(val));
    }

    let body_renamed = apply_symbol_rename(body, &rename);
    let body_bound = apply_bindings(&body_renamed, &body_env);
    let mut out = evaluate_depth_outcome(&body_bound, depth + 1);
    diagnostics.append(&mut out.diagnostics);
    out.diagnostics = diagnostics;
    if out.diagnostics.iter().any(|d| d.severity == Severity::Error) {
        out.status = ComputationStatus::Invalid;
        out.kind = EvalKind::Unevaluated;
    }
    out
}

fn apply_bindings(expr: &Term, env: &DefinitionMap) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) => {
            if let Some(def) = env.get(s) {
                if let Some(t) = definition_term(def) {
                    return t;
                }
            }
            clone_term(expr)
        }
        Term::Atom(_) => clone_term(expr),
        Term::List(items) => Term::List(items.iter().map(|i| apply_bindings(i, env)).collect()),
        Term::Application { head, arguments: args } => {
            // Do not substitute into Set / SetDelayed LHS.
            if (head.is_symbol("Set") || head.is_symbol("SetDelayed")) && args.len() == 2 {
                return Term::Application {
                    head: Box::new(clone_term(head)),
                    arguments: vec![clone_term(&args[0]), apply_bindings(&args[1], env)],
                };
            }
            let app = Term::Application {
                head: Box::new(apply_bindings(head, env)),
                arguments: args.iter().map(|a| apply_bindings(a, env)).collect(),
            };
            if let Some(rewritten) = try_apply_down_value(&app, env) { apply_bindings(&rewritten, env) } else { app }
        }
    }
}

fn try_apply_down_value(expr: &Term, env: &DefinitionMap) -> Option<Term> {
    let Term::Application { head, .. } = expr
    else {
        return None;
    };
    let Term::Atom(Atom::Symbol(name)) = head.as_ref()
    else {
        return None;
    };
    let Definition::DownValues(rules) = env.get(name)?
    else {
        return None;
    };
    for (lhs, rhs) in rules {
        if let Some(binds) = pattern_bind(expr, lhs) {
            return Some(substitute_pattern_binds(rhs, &binds));
        }
    }
    None
}

fn pattern_bind(expr: &Term, pattern: &Term) -> Option<std::collections::HashMap<String, Term>> {
    let mut binds = std::collections::HashMap::new();
    if pattern_bind_into(expr, pattern, &mut binds) { Some(binds) } else { None }
}

fn pattern_bind_into(expr: &Term, pattern: &Term, binds: &mut std::collections::HashMap<String, Term>) -> bool {
    match pattern {
        Term::Application { head, arguments: args } if head.is_symbol("Blank") => match args.as_slice() {
            [] => true,
            [head_pat] => expr_has_head(expr, head_pat),
            _ => false,
        },
        Term::Application { head, arguments: args } if head.is_symbol("Pattern") && args.len() == 2 => {
            if let Term::Atom(Atom::Symbol(name)) = &args[0] {
                if pattern_bind_into(expr, &args[1], binds) {
                    binds.insert(name.clone(), clone_term(expr));
                    true
                }
                else {
                    false
                }
            }
            else {
                false
            }
        }
        Term::List(ps) => match expr {
            Term::List(xs) if xs.len() == ps.len() => xs.iter().zip(ps.iter()).all(|(x, p)| pattern_bind_into(x, p, binds)),
            _ => false,
        },
        Term::Application { head: ph, arguments: pa } => match expr {
            Term::Application { head: eh, arguments: ea } if ea.len() == pa.len() => {
                pattern_bind_into(eh, ph, binds) && ea.iter().zip(pa.iter()).all(|(e, p)| pattern_bind_into(e, p, binds))
            }
            _ => false,
        },
        other => terms_structurally_equal(expr, other),
    }
}

fn substitute_pattern_binds(expr: &Term, binds: &std::collections::HashMap<String, Term>) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) => {
            if let Some(v) = binds.get(s) {
                clone_term(v)
            }
            else {
                clone_term(expr)
            }
        }
        Term::Atom(_) => clone_term(expr),
        Term::List(items) => Term::List(items.iter().map(|i| substitute_pattern_binds(i, binds)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(substitute_pattern_binds(head, binds)),
            arguments: args.iter().map(|a| substitute_pattern_binds(a, binds)).collect(),
        },
    }
}

fn expand_span_args(args: &[Term]) -> Option<Term> {
    let ints: Option<Vec<i64>> = args.iter().map(|t| number_from_term(t).and_then(|n| n.as_exact_integer())).collect();
    let ints = ints?;
    match ints.as_slice() {
        [a, b] => {
            let mut out = Vec::new();
            if *a <= *b {
                let mut x = *a;
                while x <= *b {
                    out.push(Term::int(x));
                    x += 1;
                }
            }
            else {
                let mut x = *a;
                while x >= *b {
                    out.push(Term::int(x));
                    x -= 1;
                }
            }
            Some(Term::List(out))
        }
        [a, step, b] => {
            if *step == 0 {
                return None;
            }
            let mut out = Vec::new();
            let mut x = *a;
            if *step > 0 {
                while x <= *b {
                    out.push(Term::int(x));
                    x += *step;
                }
            }
            else {
                while x >= *b {
                    out.push(Term::int(x));
                    x += *step;
                }
            }
            Some(Term::List(out))
        }
        _ => None,
    }
}

fn apply_builtin_outcome(head: &Term, args: Vec<Term>, depth: u32) -> EvalOutcome {
    // Pure function application: `Function[…][args…]` (head is not a bare symbol).
    if let Term::Application { head: fh, arguments: fargs } = head {
        if fh.is_symbol("Function") {
            return apply_function(fargs, &args, depth);
        }
    }

    let name = match head {
        Term::Atom(Atom::Symbol(s)) => s.as_str(),
        _ => {
            return EvalOutcome::unevaluated(Term::Application { head: Box::new(clone_term(head)), arguments: args });
        }
    };

    match name {
        "Plus" => EvalOutcome::value(eval_plus(args)),
        "Times" => eval_times_outcome(args),
        "Power" if args.len() == 2 => EvalOutcome::value(eval_power(clone_term(&args[0]), clone_term(&args[1]))),
        "Subtract" if args.len() == 2 => {
            EvalOutcome::value(eval_plus(vec![clone_term(&args[0]), eval_times(vec![Term::int(-1), clone_term(&args[1])])]))
        }
        "Divide" if args.len() == 2 => {
            EvalOutcome::value(eval_times(vec![clone_term(&args[0]), eval_power(clone_term(&args[1]), Term::int(-1))]))
        }
        "DotTimes" if args.len() == 2 => eval_dot_binop("DotTimes", &args[0], &args[1], |a, b| eval_times(vec![a, b])),
        "DotDivide" if args.len() == 2 => {
            eval_dot_binop("DotDivide", &args[0], &args[1], |a, b| eval_times(vec![a, eval_power(b, Term::int(-1))]))
        }
        "DotPower" if args.len() == 2 => eval_dot_binop("DotPower", &args[0], &args[1], eval_power),
        "Mldivide" | "DotLeftDivide" if args.len() == 2 => eval_mldivide(name, &args[0], &args[1]),
        "Mldivide" | "DotLeftDivide" => {
            let term = Term::Application { head: Box::new(Term::symbol(name)), arguments: args };
            EvalOutcome::invalid(term, unsupported_operation(name))
        }
        "Span" if args.len() == 2 || args.len() == 3 => match expand_span_args(&args) {
            Some(list) => EvalOutcome::value(list),
            None => EvalOutcome::unevaluated(Term::apply("Span", args)),
        },
        "Range" => eval_range(args),
        "Length" if args.len() == 1 => eval_length(&args[0]),
        "First" if args.len() == 1 => eval_first(&args[0]),
        "Join" => eval_join(args),
        "Apply" if args.len() == 2 => eval_apply(&args[0], &args[1], depth),
        "Zeros" => eval_matrix_fill("Zeros", &args, 0),
        "Ones" => eval_matrix_fill("Ones", &args, 1),
        "Eye" | "IdentityMatrix" => eval_eye(name, args),
        "Size" | "Dimensions" if args.len() == 1 => eval_size(&args[0]),
        "Det" if args.len() == 1 => eval_det(&args[0]),
        "LinearSolve" if args.len() == 2 => eval_mldivide("LinearSolve", &args[0], &args[1]),
        "Solve" if args.len() == 2 => eval_solve(&args[0], &args[1]),
        "List" => EvalOutcome::value(Term::List(args)),
        "Simplify" if args.len() == 1 => EvalOutcome::value(eval_simplify(&args[0], depth)),
        "Sin" | "Cos" | "Tan" | "Exp" | "Log" if args.len() == 1 => EvalOutcome::value(eval_machine_unary(name, &args[0])),
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
        "Less" if args.len() == 2 => wrap_compare(eval_compare("Less", &args[0], &args[1], |o| o == Ordering::Less), "Less"),
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
            let term = Term::Application { head: Box::new(clone_term(head)), arguments: args };
            if let Some(req) = crate::calculus::try_calculus_request(&term) {
                let result = crate::calculus::execute_calculus(req);
                return EvalOutcome::value(crate::calculus::calculus_result_bridge_term(&result));
            }
            EvalOutcome::unevaluated(term)
        }
        "Function" => EvalOutcome::unevaluated(Term::Application { head: Box::new(Term::symbol("Function")), arguments: args }),
        "ReplaceAll" if args.len() == 2 => EvalOutcome::value(eval_replace_all(&args[0], &args[1], depth)),
        "Part" if args.len() >= 2 => eval_part_n(&args),
        "Rule" | "RuleDelayed" if args.len() == 2 => {
            EvalOutcome::unevaluated(Term::Application { head: Box::new(clone_term(head)), arguments: args })
        }
        "Import" | "Export" | "Clear" | "Timing" => {
            let term = Term::Application { head: Box::new(Term::symbol(name)), arguments: args };
            EvalOutcome::invalid(term, unsupported_operation(name))
        }
        "error" | "Error" => {
            let msg = match args.first() {
                Some(Term::Atom(Atom::String(s))) => s.clone(),
                _ => "error".to_string(),
            };
            let term = Term::Application { head: Box::new(Term::symbol(name)), arguments: args };
            EvalOutcome::invalid(
                term,
                Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", "error").detail("message", msg),
            )
        }
        _ => EvalOutcome::unevaluated(Term::Application { head: Box::new(clone_term(head)), arguments: args }),
    }
}

/// Apply `Function[body]` (Slot) or `Function[var, body]` (named) to arguments.
fn apply_function(fargs: &[Term], args: &[Term], depth: u32) -> EvalOutcome {
    match fargs {
        [body] if args.len() == 1 => {
            let substituted = substitute_slot(body, &args[0]);
            evaluate_depth_outcome(&substituted, depth + 1)
        }
        [Term::Atom(Atom::Symbol(var)), body] if args.len() == 1 => {
            let substituted = substitute_symbol(body, var, &args[0]);
            evaluate_depth_outcome(&substituted, depth + 1)
        }
        _ => EvalOutcome::unevaluated(Term::Application {
            head: Box::new(Term::apply("Function", clone_terms(fargs))),
            arguments: clone_terms(args),
        }),
    }
}

fn wrap_compare(term: Term, head: &str) -> EvalOutcome {
    if matches!(&term, Term::Application { head: h, .. } if h.is_symbol(head)) {
        EvalOutcome::unevaluated(term)
    }
    else {
        EvalOutcome::value(term)
    }
}

fn wrap_logic(term: Term, head: &str) -> EvalOutcome {
    if matches!(&term, Term::Application { head: h, .. } if h.is_symbol(head)) {
        EvalOutcome::unevaluated(term)
    }
    else {
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

/// `Times`：标量 × nested List 按 MATLAB/数组语义广播；两矩阵走 `matmul`；其它保持符号 `Times`。
fn eval_times_outcome(args: Vec<Term>) -> EvalOutcome {
    let mut scalars = Vec::new();
    let mut lists = Vec::new();
    let mut other = false;
    for a in &args {
        if number_from_term(a).is_some() {
            scalars.push(clone_term(a));
        }
        else if matches!(a, Term::List(_)) {
            lists.push(clone_term(a));
        }
        else {
            other = true;
            break;
        }
    }
    if !other && lists.len() == 1 && !scalars.is_empty() && scalars.len() + 1 == args.len() {
        let scale = eval_times(scalars);
        return eval_dot_binop("DotTimes", &scale, &lists[0], |a, b| eval_times(vec![a, b]));
    }
    if !other && lists.len() == 2 && scalars.is_empty() && args.len() == 2 {
        if let (Some(am), Some(bm)) = (term_to_rational_matrix(&lists[0]), term_to_rational_matrix(&lists[1])) {
            return match matmul(&am, &bm) {
                Ok(m) => match matrix_to_nested_list(&m) {
                    Ok(term) => EvalOutcome::value(term),
                    Err(d) => EvalOutcome::invalid(Term::apply("Times", args), d),
                },
                Err(d) => EvalOutcome::invalid(Term::apply("Times", args), d),
            };
        }
    }
    EvalOutcome::value(eval_times(args))
}

fn eval_det(arg: &Term) -> EvalOutcome {
    let echo = || Term::apply("Det", vec![clone_term(arg)]);
    let Some(m) = term_to_rational_matrix(arg)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    match det_bareiss(&m) {
        Ok(r) => EvalOutcome::value(rational_to_term(&r.det)),
        Err(d) => EvalOutcome::invalid(echo(), d),
    }
}

/// MATLAB-style `sum`：向量 → 标量和；矩阵 → 各列之和（行向量）。
fn eval_array_sum(arg: &Term) -> EvalOutcome {
    let echo = || Term::apply("Sum", vec![clone_term(arg)]);
    let Some(m) = term_to_rational_matrix(arg)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    let rows = m.shape().rows;
    let cols = m.shape().cols;
    let entry_q = |i: u64, j: u64| -> std::result::Result<Rational, Diagnostic> {
        match m.get(i, j)? {
            crate::linear_algebra::MatrixEntry::Rational(r) => Ok(r),
            crate::linear_algebra::MatrixEntry::Integer(n) => Ok(Rational::from_integer(n)),
            crate::linear_algebra::MatrixEntry::MachineF64(_) => {
                Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "sum_requires_exact"))
            }
        }
    };
    if rows == 1 || cols == 1 {
        let mut acc = Rational::new(Integer::from_i64(0), Integer::one());
        for i in 0..rows {
            for j in 0..cols {
                match entry_q(i, j) {
                    Ok(r) => acc = acc.add(&r),
                    Err(d) => return EvalOutcome::invalid(echo(), d),
                }
            }
        }
        return EvalOutcome::value(rational_to_term(&acc));
    }
    let mut out = Vec::with_capacity(cols as usize);
    for j in 0..cols {
        let mut acc = Rational::new(Integer::from_i64(0), Integer::one());
        for i in 0..rows {
            match entry_q(i, j) {
                Ok(r) => acc = acc.add(&r),
                Err(d) => return EvalOutcome::invalid(echo(), d),
            }
        }
        out.push(rational_to_term(&acc));
    }
    EvalOutcome::value(Term::List(out))
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
            // Scalar `x^0 → 1`. Lists use `DotPower` for elementwise; matrix `A^0` is not this path.
            if matches!(&base, Term::List(_)) {
                return Term::apply("Power", vec![base, exp]);
            }
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
                            let rest = if args.len() == 2 {
                                clone_term(&args[1])
                            }
                            else {
                                Term::apply("Times", clone_terms(&args[1..]))
                            };
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
    if let Some(exact) = eval_trig_exact(name, arg) {
        return exact;
    }
    let Some(x) = term_as_f64(arg)
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

/// Exact trig values for Feature Gap (`Cos[Pi]`, `Sin[0]`, …) before machine float.
fn eval_trig_exact(name: &str, arg: &Term) -> Option<Term> {
    let angle = normalize_pi_angle(arg)?;
    // `angle` is integer multiple of `$\pi$` (k).
    match name {
        "Sin" => {
            // sin(k π) = 0
            Some(Term::int(0))
        }
        "Cos" => {
            // cos(k π) = (-1)^k
            if angle % 2 == 0 { Some(Term::int(1)) } else { Some(Term::int(-1)) }
        }
        "Tan" if angle % 2 == 0 => Some(Term::int(0)),
        _ => None,
    }
}

/// Map `0`, `Pi`, `-Pi`, `n*Pi` to integer `n` when possible.
fn normalize_pi_angle(arg: &Term) -> Option<i64> {
    if let Some(n) = number_from_term(arg).and_then(|n| n.as_exact_integer()) {
        if n == 0 {
            return Some(0);
        }
    }
    if arg.is_symbol("Pi") {
        return Some(1);
    }
    match arg {
        Term::Application { head, arguments: args } if head.is_symbol("Times") => match args.as_slice() {
            [a, b] if a.is_symbol("Pi") => number_from_term(b).and_then(|n| n.as_exact_integer()),
            [a, b] if b.is_symbol("Pi") => number_from_term(a).and_then(|n| n.as_exact_integer()),
            _ => None,
        },
        Term::Application { head, arguments: args } if head.is_symbol("Plus") && args.len() == 1 && args[0].is_symbol("Pi") => {
            Some(1)
        }
        _ => None,
    }
}

fn term_as_f64(arg: &Term) -> Option<f64> {
    if let Some(k) = normalize_pi_angle(arg) {
        return Some((k as f64) * std::f64::consts::PI);
    }
    if arg.is_symbol("E") {
        return Some(std::f64::consts::E);
    }
    number_from_term(arg).and_then(|n| num_to_f64_lossy(&n))
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
    F: Fn(Ordering) -> bool + Copy,
{
    match (left, right) {
        (Term::List(xs), Term::List(ys)) if xs.len() == ys.len() => {
            Term::List(xs.iter().zip(ys.iter()).map(|(a, b)| eval_compare(head, a, b, cmp)).collect())
        }
        (Term::List(xs), r) => Term::List(xs.iter().map(|a| eval_compare(head, a, r, cmp)).collect()),
        (l, Term::List(ys)) => Term::List(ys.iter().map(|b| eval_compare(head, l, b, cmp)).collect()),
        _ => {
            if let (Some(a), Some(b)) = (number_from_term(left), number_from_term(right)) {
                if let Some(ord) = num_compare(a, b) {
                    return Term::boolean(cmp(ord));
                }
            }
            Term::apply(head, vec![clone_term(left), clone_term(right)])
        }
    }
}

fn eval_logic_and(left: &Term, right: &Term) -> Term {
    match (as_boolean(left), as_boolean(right)) {
        (Some(a), Some(b)) => Term::boolean(a && b),
        _ => Term::apply("And", vec![clone_term(left), clone_term(right)]),
    }
}

fn eval_logic_or(left: &Term, right: &Term) -> Term {
    match (as_boolean(left), as_boolean(right)) {
        (Some(a), Some(b)) => Term::boolean(a || b),
        _ => Term::apply("Or", vec![clone_term(left), clone_term(right)]),
    }
}

fn eval_logic_not(arg: &Term) -> Term {
    match as_boolean(arg) {
        Some(v) => Term::boolean(!v),
        None => Term::apply("Not", vec![clone_term(arg)]),
    }
}

fn eval_range(args: Vec<Term>) -> EvalOutcome {
    let ints: Option<Vec<i64>> = args.iter().map(|t| number_from_term(t).and_then(|n| n.as_exact_integer())).collect();
    let Some(ints) = ints
    else {
        return EvalOutcome::unevaluated(Term::apply("Range", args));
    };
    let list = match ints.as_slice() {
        [n] => range_ints(1, *n, 1),
        [a, b] => range_ints(*a, *b, 1),
        [a, b, step] => range_ints(*a, *b, *step),
        _ => None,
    };
    match list {
        Some(items) => EvalOutcome::value(Term::List(items)),
        None => EvalOutcome::unevaluated(Term::apply("Range", args)),
    }
}

fn eval_length(arg: &Term) -> EvalOutcome {
    match arg {
        Term::List(items) => EvalOutcome::value(Term::int(items.len() as i64)),
        Term::Application { arguments, .. } => EvalOutcome::value(Term::int(arguments.len() as i64)),
        _ => EvalOutcome::unevaluated(Term::apply("Length", vec![clone_term(arg)])),
    }
}

fn eval_first(arg: &Term) -> EvalOutcome {
    match arg {
        Term::List(items) if !items.is_empty() => EvalOutcome::value(clone_term(&items[0])),
        Term::Application { arguments, .. } if !arguments.is_empty() => EvalOutcome::value(clone_term(&arguments[0])),
        Term::List(items) => EvalOutcome::invalid(
            Term::apply("First", vec![clone_term(arg)]),
            invalid_index_diagnostic(1, Some(items.len() as u64)),
        ),
        Term::Application { arguments, .. } => EvalOutcome::invalid(
            Term::apply("First", vec![clone_term(arg)]),
            invalid_index_diagnostic(1, Some(arguments.len() as u64)),
        ),
        _ => EvalOutcome::unevaluated(Term::apply("First", vec![clone_term(arg)])),
    }
}

fn eval_join(args: Vec<Term>) -> EvalOutcome {
    let mut out = Vec::new();
    for arg in &args {
        match arg {
            Term::List(items) => out.extend(items.iter().map(clone_term)),
            _ => return EvalOutcome::unevaluated(Term::apply("Join", args)),
        }
    }
    EvalOutcome::value(Term::List(out))
}

fn eval_apply(func: &Term, target: &Term, depth: u32) -> EvalOutcome {
    match target {
        Term::List(items) => {
            let app = Term::Application { head: Box::new(clone_term(func)), arguments: clone_terms(items) };
            evaluate_depth_outcome(&app, depth + 1)
        }
        other => EvalOutcome::unevaluated(Term::apply("Apply", vec![clone_term(func), clone_term(other)])),
    }
}

/// Parse `n` or `m,n` non-negative integer dimensions for matrix constructors.
fn parse_matrix_dims(args: &[Term]) -> Option<(u64, u64)> {
    let as_dim = |t: &Term| -> Option<u64> {
        let n = number_from_term(t)?.as_exact_integer()?;
        if n < 0 { None } else { Some(n as u64) }
    };
    match args {
        [n] => {
            let n = as_dim(n)?;
            Some((n, n))
        }
        [m, n] => Some((as_dim(m)?, as_dim(n)?)),
        _ => None,
    }
}

fn matrix_fill_nested(rows: u64, cols: u64, fill: i64) -> EvalOutcome {
    let n = match rows.checked_mul(cols) {
        Some(v) => v as usize,
        None => {
            return EvalOutcome::invalid(
                Term::List(vec![]),
                Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "dims_overflow"),
            );
        }
    };
    let fill_r = Rational::new(Integer::from_i64(fill), Integer::one());
    let mut data = Vec::with_capacity(n);
    for _ in 0..n {
        data.push(clone_rational(&fill_r));
    }
    match MatrixValue::from_rationals_row_major(rows, cols, data) {
        Ok(m) => match matrix_to_nested_list(&m) {
            Ok(term) => EvalOutcome::value(term),
            Err(d) => EvalOutcome::invalid(Term::List(vec![]), d),
        },
        Err(d) => EvalOutcome::invalid(Term::List(vec![]), d),
    }
}

fn eval_matrix_fill(head: &str, args: &[Term], fill: i64) -> EvalOutcome {
    let Some((rows, cols)) = parse_matrix_dims(args)
    else {
        return EvalOutcome::unevaluated(Term::apply(head, clone_terms(args)));
    };
    matrix_fill_nested(rows, cols, fill)
}

fn eval_eye(head: &str, args: Vec<Term>) -> EvalOutcome {
    let Some((rows, cols)) = parse_matrix_dims(&args)
    else {
        return EvalOutcome::unevaluated(Term::apply(head, args));
    };
    let n = match rows.checked_mul(cols) {
        Some(v) => v as usize,
        None => {
            return EvalOutcome::invalid(
                Term::apply(head, args),
                Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "dims_overflow"),
            );
        }
    };
    let zero = Rational::new(Integer::from_i64(0), Integer::one());
    let one = Rational::new(Integer::from_i64(1), Integer::one());
    let mut data = Vec::with_capacity(n);
    for _ in 0..n {
        data.push(clone_rational(&zero));
    }
    let diag = rows.min(cols);
    for i in 0..diag {
        data[(i * cols + i) as usize] = clone_rational(&one);
    }
    match MatrixValue::from_rationals_row_major(rows, cols, data) {
        Ok(m) => match matrix_to_nested_list(&m) {
            Ok(term) => EvalOutcome::value(term),
            Err(d) => EvalOutcome::invalid(Term::apply(head, args), d),
        },
        Err(d) => EvalOutcome::invalid(Term::apply(head, args), d),
    }
}

/// Shape of nested-list matrices / row vectors for Feature Gap Term bridge.
fn nested_list_shape(term: &Term) -> Option<(u64, u64)> {
    match term {
        Term::List(rows) if rows.is_empty() => Some((0, 0)),
        Term::List(rows) if matches!(rows.first(), Some(Term::List(_))) => {
            let mut cols: Option<u64> = None;
            for row in rows {
                let Term::List(cells) = row
                else {
                    return None;
                };
                let c = cells.len() as u64;
                match cols {
                    Some(prev) if prev != c => return None,
                    None => cols = Some(c),
                    _ => {}
                }
            }
            Some((rows.len() as u64, cols.unwrap_or(0)))
        }
        Term::List(cells) => Some((1, cells.len() as u64)),
        _ => None,
    }
}

fn eval_size(arg: &Term) -> EvalOutcome {
    match nested_list_shape(arg) {
        Some((rows, cols)) => EvalOutcome::value(Term::List(vec![Term::int(rows as i64), Term::int(cols as i64)])),
        None => EvalOutcome::unevaluated(Term::apply("Size", vec![clone_term(arg)])),
    }
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
        Term::Application { head, arguments: args }
            if head.is_symbol("Function") && args.len() == 2 && matches!(&args[0], Term::Atom(Atom::Symbol(_))) =>
        {
            if let Term::Atom(Atom::Symbol(var)) = &args[0] {
                substitute_symbol(&args[1], var, item)
            }
            else {
                Term::apply("Map", vec![clone_term(func), clone_term(item)])
            }
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

fn is_end_symbol(term: &Term) -> bool {
    matches!(term, Term::Atom(Atom::Symbol(s)) if s == "End" || s == "end")
}

fn is_all_symbol(term: &Term) -> bool {
    matches!(term, Term::Atom(Atom::Symbol(s)) if s == "All" || s == ":")
}

fn eval_part_n(args: &[Term]) -> EvalOutcome {
    if args.len() < 2 {
        return EvalOutcome::unevaluated(Term::apply("Part", clone_terms(args)));
    }

    // `Part[m, All, j, …]` — map remaining indices over each row (MATLAB `A(:,j)`).
    if is_all_symbol(&args[1]) && args.len() >= 3 {
        if let Term::List(rows) = &args[0] {
            let mut diagnostics = Vec::new();
            let mut out = Vec::with_capacity(rows.len());
            for row in rows {
                let mut part_args = Vec::with_capacity(args.len() - 1);
                part_args.push(clone_term(row));
                for index in &args[2..] {
                    part_args.push(clone_term(index));
                }
                let mut o = eval_part_n(&part_args);
                let errored = o.has_error();
                diagnostics.append(&mut o.diagnostics);
                if errored {
                    o.diagnostics = diagnostics;
                    return o;
                }
                out.push(o.term);
            }
            return EvalOutcome { term: Term::List(out), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics };
        }
    }

    let mut cur = clone_term(&args[0]);
    let mut diagnostics = Vec::new();
    for index in &args[1..] {
        let mut o = eval_part_outcome(&cur, index);
        let errored = o.has_error();
        diagnostics.append(&mut o.diagnostics);
        if errored {
            o.diagnostics = diagnostics;
            return o;
        }
        cur = o.term;
    }
    EvalOutcome { term: cur, kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
}

fn eval_dot_binop<F>(head: &str, left: &Term, right: &Term, op: F) -> EvalOutcome
where
    F: Fn(Term, Term) -> Term + Copy,
{
    match (left, right) {
        (Term::List(a), Term::List(b)) if a.len() == b.len() => {
            let mut out = Vec::with_capacity(a.len());
            let mut diagnostics = Vec::new();
            for (x, y) in a.iter().zip(b.iter()) {
                let mut cell = if matches!(x, Term::List(_)) || matches!(y, Term::List(_)) {
                    eval_dot_binop(head, x, y, op)
                }
                else {
                    EvalOutcome::value(op(clone_term(x), clone_term(y)))
                };
                if cell.has_error() {
                    diagnostics.append(&mut cell.diagnostics);
                    return EvalOutcome {
                        term: Term::apply(head, vec![clone_term(left), clone_term(right)]),
                        kind: EvalKind::Unevaluated,
                        status: ComputationStatus::Invalid,
                        diagnostics,
                    };
                }
                diagnostics.append(&mut cell.diagnostics);
                out.push(cell.term);
            }
            EvalOutcome { term: Term::List(out), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
        }
        (Term::List(a), Term::List(b)) => EvalOutcome::invalid(
            Term::apply(head, vec![clone_term(left), clone_term(right)]),
            Diagnostic::new(DiagnosticCode::ShapeMismatch)
                .detail("reason", "elementwise_length_mismatch")
                .detail("left", a.len().to_string())
                .detail("right", b.len().to_string()),
        ),
        (Term::List(a), b) => {
            let mut out = Vec::with_capacity(a.len());
            let mut diagnostics = Vec::new();
            for x in a {
                let mut cell = if matches!(x, Term::List(_)) {
                    eval_dot_binop(head, x, b, op)
                }
                else {
                    EvalOutcome::value(op(clone_term(x), clone_term(b)))
                };
                if cell.has_error() {
                    diagnostics.append(&mut cell.diagnostics);
                    return EvalOutcome {
                        term: Term::apply(head, vec![clone_term(left), clone_term(right)]),
                        kind: EvalKind::Unevaluated,
                        status: ComputationStatus::Invalid,
                        diagnostics,
                    };
                }
                diagnostics.append(&mut cell.diagnostics);
                out.push(cell.term);
            }
            EvalOutcome { term: Term::List(out), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
        }
        (a, Term::List(b)) => {
            let mut out = Vec::with_capacity(b.len());
            let mut diagnostics = Vec::new();
            for y in b {
                let mut cell = if matches!(y, Term::List(_)) {
                    eval_dot_binop(head, a, y, op)
                }
                else {
                    EvalOutcome::value(op(clone_term(a), clone_term(y)))
                };
                if cell.has_error() {
                    diagnostics.append(&mut cell.diagnostics);
                    return EvalOutcome {
                        term: Term::apply(head, vec![clone_term(left), clone_term(right)]),
                        kind: EvalKind::Unevaluated,
                        status: ComputationStatus::Invalid,
                        diagnostics,
                    };
                }
                diagnostics.append(&mut cell.diagnostics);
                out.push(cell.term);
            }
            EvalOutcome { term: Term::List(out), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics }
        }
        (a, b) if number_from_term(a).is_some() && number_from_term(b).is_some() => {
            EvalOutcome::value(op(clone_term(a), clone_term(b)))
        }
        _ => EvalOutcome::unevaluated(Term::apply(head, vec![clone_term(left), clone_term(right)])),
    }
}

fn eval_part_outcome(expr: &Term, index: &Term) -> EvalOutcome {
    if let Term::List(indices) = index {
        let mut out = Vec::with_capacity(indices.len());
        let mut diagnostics = Vec::new();
        for idx in indices {
            let mut o = eval_part_outcome(expr, idx);
            let errored = o.has_error();
            diagnostics.append(&mut o.diagnostics);
            if errored {
                o.diagnostics = diagnostics;
                return o;
            }
            out.push(o.term);
        }
        return EvalOutcome { term: Term::List(out), kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics };
    }

    // MATLAB `end` / `All` (`:`) relative to the current list.
    if let Term::List(items) = expr {
        if is_end_symbol(index) {
            return eval_part_outcome(expr, &Term::int(items.len() as i64));
        }
        if is_all_symbol(index) {
            return EvalOutcome::value(Term::List(items.iter().map(clone_term).collect()));
        }
    }

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
            }
            else {
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
        Term::Application { head, arguments: args }
            if head.is_symbol("Slot")
                && (args.is_empty()
                    || (args.len() == 1
                        && matches!(&args[0], Term::Atom(Atom::Number(n)) if n.as_exact_integer() == Some(1)))) =>
        {
            clone_term(value)
        }
        Term::Atom(_) => clone_term(body),
        Term::List(items) => Term::List(items.iter().map(|i| substitute_slot(i, value)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(substitute_slot(head, value)),
            arguments: args.iter().map(|a| substitute_slot(a, value)).collect(),
        },
    }
}

/// Living 25 Term bridge：单变量多项式 `Solve` → `{{x->r},…}`（typed `SolutionSet` 仍为正式合同）。
fn eval_solve(equation: &Term, unknown: &Term) -> EvalOutcome {
    let echo = || Term::apply("Solve", vec![clone_term(equation), clone_term(unknown)]);
    let Term::Atom(Atom::Symbol(var)) = unknown
    else {
        return EvalOutcome::unevaluated(echo());
    };
    let zero_expr = match equation {
        Term::Application { head, arguments } if head.is_symbol("Equal") && arguments.len() == 2 => evaluate(&Term::apply(
            "Plus",
            vec![clone_term(&arguments[0]), Term::apply("Times", vec![Term::int(-1), clone_term(&arguments[1])])],
        )),
        other => evaluate(other),
    };
    let Some(terms) = collect_univariate_monomials(&zero_expr, var)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    if terms.is_empty() {
        // 0 == 0 → 恒真，无离散根集；保守未求值。
        return EvalOutcome::unevaluated(echo());
    }

    use crate::{
        polynomial::{CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialFactorLimits, RingTable},
        solve::{BoundSymbol, CoverageStatus, SolveDomain, solve_univariate_polynomial_roots},
    };
    use athena_types::SymbolId;

    let mut rings = RingTable::new();
    let Ok(ring) = rings.intern(CoefficientDomain::Rational, vec![SymbolId(0)], MonomialOrder::Lex)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    let mut builder = PolynomialBuilder::new(ring);
    for (coeff, deg) in terms {
        if let Err(_) = builder.push_term(coeff, vec![deg]) {
            return EvalOutcome::unevaluated(echo());
        }
    }
    let Ok(poly) = builder.build(&rings)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    let unknown_sym = BoundSymbol::free(SymbolId(0));
    let Ok(adapted) =
        solve_univariate_polynomial_roots(poly, &rings, unknown_sym, SolveDomain::Rationals, PolynomialFactorLimits::default())
    else {
        return EvalOutcome::unevaluated(echo());
    };
    if !matches!(adapted.solution.coverage, CoverageStatus::Complete) {
        return EvalOutcome::unevaluated(echo());
    }

    let mut roots: Vec<Term> = Vec::new();
    for branch in &adapted.solution.branches {
        let Some(tid) = branch.bindings.get(&unknown_sym)
        else {
            return EvalOutcome::unevaluated(echo());
        };
        let Some(val) = adapted.values.get(tid)
        else {
            return EvalOutcome::unevaluated(echo());
        };
        let root_term = match val {
            crate::solve::BindingValue::Number(n) => number_to_term(n),
            crate::solve::BindingValue::Rational(r) => rational_to_term(r),
            crate::solve::BindingValue::MachineF64(_) => return EvalOutcome::unevaluated(echo()),
        };
        roots.push(root_term);
    }
    roots.sort_by(|a, b| match (number_from_term(a), number_from_term(b)) {
        (Some(na), Some(nb)) => num_compare(&na, &nb).unwrap_or(Ordering::Equal),
        _ => Ordering::Equal,
    });

    let list =
        Term::List(roots.into_iter().map(|r| Term::List(vec![Term::apply("Rule", vec![Term::symbol(var), r])])).collect());
    EvalOutcome::value(list)
}

fn number_to_term(n: &Number) -> Term {
    if let Some(i) = n.as_exact_integer() {
        return Term::int(i);
    }
    if let Some(i) = n.as_integer() {
        if let Some(v) = i.to_i64() {
            return Term::int(v);
        }
    }
    if let Some(r) = n.as_rational() {
        return rational_to_term(r);
    }
    Term::number(clone_number(n))
}

/// 将 `Plus`/`Times`/`Power` 展开为单变量 `(coeff, degree)` 项（仅有理系数）。
pub(crate) fn collect_univariate_monomials_for_solve(expr: &Term, var: &str) -> Option<Vec<(Number, u32)>> {
    collect_univariate_monomials(expr, var)
}

fn collect_univariate_monomials(expr: &Term, var: &str) -> Option<Vec<(Number, u32)>> {
    fn merge(dst: &mut Vec<(Number, u32)>, src: Vec<(Number, u32)>) -> Option<()> {
        for (c, d) in src {
            if let Some((existing, _)) = dst.iter_mut().find(|(_, ed)| *ed == d) {
                *existing = num_add(clone_number(existing), c).ok()?;
            }
            else {
                dst.push((c, d));
            }
        }
        dst.retain(|(c, _)| !c.is_zero());
        Some(())
    }

    fn mul_lists(a: &[(Number, u32)], b: &[(Number, u32)]) -> Option<Vec<(Number, u32)>> {
        let mut out = Vec::new();
        for (ca, da) in a {
            for (cb, db) in b {
                let c = num_mul(clone_number(ca), clone_number(cb)).ok()?;
                let d = da.checked_add(*db)?;
                merge(&mut out, vec![(c, d)])?;
            }
        }
        Some(out)
    }

    fn go(expr: &Term, var: &str) -> Option<Vec<(Number, u32)>> {
        match expr {
            Term::Atom(Atom::Symbol(s)) if s.as_str() == var => Some(vec![(Number::small_int(1), 1)]),
            Term::Atom(_) => {
                let n = number_from_term(expr)?;
                Some(vec![(clone_number(n), 0)])
            }
            Term::Application { head, arguments } if head.is_symbol("Plus") => {
                let mut out = Vec::new();
                for a in arguments {
                    merge(&mut out, go(a, var)?)?;
                }
                Some(out)
            }
            Term::Application { head, arguments } if head.is_symbol("Times") => {
                let mut out = vec![(Number::small_int(1), 0)];
                for a in arguments {
                    out = mul_lists(&out, &go(a, var)?)?;
                }
                Some(out)
            }
            Term::Application { head, arguments } if head.is_symbol("Power") && arguments.len() == 2 => {
                let exp = number_from_term(&arguments[1])?.as_integer_exp()?;
                if exp < 0 {
                    return None;
                }
                let exp = exp as u32;
                if arguments[0].is_symbol(var) {
                    return Some(vec![(Number::small_int(1), exp)]);
                }
                let base = go(&arguments[0], var)?;
                if base.len() == 1 && base[0].1 == 0 {
                    let mut p = Number::small_int(1);
                    for _ in 0..exp {
                        p = num_mul(p, clone_number(&base[0].0)).ok()?;
                    }
                    return Some(vec![(p, 0)]);
                }
                // (poly)^n：仅支持已展开低次，保守拒绝。
                if exp == 0 {
                    return Some(vec![(Number::small_int(1), 0)]);
                }
                if exp == 1 {
                    return Some(base);
                }
                None
            }
            Term::List(_) => None,
            other => {
                if let Some(n) = number_from_term(other) {
                    Some(vec![(clone_number(n), 0)])
                }
                else {
                    None
                }
            }
        }
    }

    go(expr, var)
}

/// Living 25 Term bridge：exact `A\b` via [`solve_exact`]. Non-numeric / singular → keep head + diagnostic.
fn eval_mldivide(head: &str, a: &Term, b: &Term) -> EvalOutcome {
    let echo = || Term::apply(head, vec![clone_term(a), clone_term(b)]);
    let Some(am) = term_to_rational_matrix(a)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    let Some(bm) = term_to_rational_matrix(b)
    else {
        return EvalOutcome::unevaluated(echo());
    };
    match solve_exact(&am, &bm) {
        Ok(sol) if sol.disposition == SolveDisposition::Unique => match sol.particular {
            Some(x) => match matrix_to_nested_list(&x) {
                Ok(term) => EvalOutcome::value(term),
                Err(d) => EvalOutcome::invalid(echo(), d),
            },
            None => EvalOutcome::invalid(echo(), unsupported_operation(head)),
        },
        Ok(sol) => {
            let detail = match sol.disposition {
                SolveDisposition::Inconsistent => "inconsistent",
                SolveDisposition::Infinite { .. } => "underdetermined",
                SolveDisposition::Unique => "unique",
                SolveDisposition::Singular => "singular",
                SolveDisposition::ResourceLimited => "resource_limited",
            };
            EvalOutcome::invalid(
                echo(),
                Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", head).detail("reason", detail),
            )
        }
        Err(d) => EvalOutcome::invalid(echo(), d),
    }
}

fn term_scalar_rational(term: &Term) -> Option<Rational> {
    let n = number_from_term(term)?;
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(crate::numeric_clone::clone_integer(i), Integer::one()));
    }
    n.as_rational().map(clone_rational)
}

fn term_to_rational_matrix(term: &Term) -> Option<MatrixValue> {
    match term {
        Term::List(rows) if !rows.is_empty() => {
            if matches!(rows.first(), Some(Term::List(_))) {
                let mut data = Vec::new();
                let mut cols: Option<u64> = None;
                for row in rows {
                    let Term::List(cells) = row
                    else {
                        return None;
                    };
                    let c = cells.len() as u64;
                    match cols {
                        Some(prev) if prev != c => return None,
                        None => cols = Some(c),
                        _ => {}
                    }
                    for cell in cells {
                        data.push(term_scalar_rational(cell)?);
                    }
                }
                let cols = cols.unwrap_or(0);
                MatrixValue::from_rationals_row_major(rows.len() as u64, cols, data).ok()
            }
            else {
                let mut data = Vec::with_capacity(rows.len());
                for cell in rows {
                    data.push(term_scalar_rational(cell)?);
                }
                MatrixValue::from_rationals_row_major(1, data.len() as u64, data).ok()
            }
        }
        other => {
            let r = term_scalar_rational(other)?;
            MatrixValue::from_rationals_row_major(1, 1, vec![r]).ok()
        }
    }
}

pub(crate) fn rational_to_term_for_solve(r: &Rational) -> Term {
    rational_to_term(r)
}

fn rational_to_term(r: &Rational) -> Term {
    if r.is_integer() {
        if let Some(i) = r.numerator().to_i64() {
            return Term::int(i);
        }
    }
    Term::number(Number::from_rational_normalized(clone_rational(r)))
}

fn matrix_to_nested_list(m: &MatrixValue) -> std::result::Result<Term, Diagnostic> {
    let rows = m.shape().rows;
    let cols = m.shape().cols;
    let mut out = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            match m.get(i, j)? {
                crate::linear_algebra::MatrixEntry::Rational(r) => row.push(rational_to_term(&r)),
                crate::linear_algebra::MatrixEntry::Integer(n) => {
                    if let Some(i64v) = n.to_i64() {
                        row.push(Term::int(i64v));
                    }
                    else {
                        row.push(Term::number(Number::integer(crate::numeric_clone::clone_integer(&n))));
                    }
                }
                crate::linear_algebra::MatrixEntry::MachineF64(x) => row.push(Term::real(x)),
            }
        }
        out.push(Term::List(row));
    }
    Ok(Term::List(out))
}
