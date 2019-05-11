//! 比较链 · 逻辑 · 三角/超越 · 列表基础 handler（legacy `eval_compare*` / `eval_logic_*` 等语义）。

use std::cmp::Ordering;

use athena_ir::{AtomKind, TermKind};
use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, TermId};

use super::{Outcome, arith::number_of, vm::Vm};

/// 将项解释为 typed Boolean（Boolean 原子 · `True`/`False` 符号 · 精确 `1`/`0`）。
pub(crate) fn as_boolean_id(vm: &Vm<'_>, id: TermId) -> Option<bool> {
    match vm.session.arena.get(id) {
        Some(TermKind::Atom(AtomKind::Boolean(b))) => Some(*b),
        Some(TermKind::Atom(AtomKind::Symbol(s))) => match vm.session.arena.symbols().resolve(*s) {
            Some("True") => Some(true),
            Some("False") => Some(false),
            _ => None,
        },
        Some(TermKind::Atom(AtomKind::Number(n))) => {
            if n.is_zero() {
                Some(false)
            }
            else if *n == Number::small_int(1) {
                Some(true)
            }
            else {
                None
            }
        }
        _ => None,
    }
}

/// 构造非法下标诊断。
pub(crate) fn invalid_index_diagnostic(index: i64, length: Option<u64>) -> Diagnostic {
    let d = Diagnostic::new(DiagnosticCode::InvalidIndex).arg("index", index);
    match length {
        Some(len) => d.arg("length", len),
        None => d,
    }
}

/// 构造非布尔条件诊断。
pub(crate) fn non_boolean_condition_diagnostic(got: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NonBooleanCondition).detail("expected", "Boolean").detail("got", got)
}

/// 诊断摘要（legacy `term_summary`）。
pub(crate) fn term_summary(vm: &Vm<'_>, term: TermId) -> String {
    match vm.session.arena.get(term) {
        Some(TermKind::Atom(AtomKind::Symbol(s))) => vm.session.arena.symbols().resolve(*s).unwrap_or("?").to_string(),
        Some(TermKind::Atom(AtomKind::String(_))) => "String".into(),
        Some(TermKind::Atom(AtomKind::Number(_))) => "Number".into(),
        Some(TermKind::Atom(AtomKind::Boolean(true))) => "True".into(),
        Some(TermKind::Atom(AtomKind::Boolean(false))) => "False".into(),
        Some(TermKind::Atom(AtomKind::Null)) => "Null".into(),
        Some(TermKind::List(_)) => "List".into(),
        Some(TermKind::App { .. }) => vm.head_name(term).unwrap_or_else(|| "Application".into()),
        None => "Invalid".into(),
    }
}

// ---- 比较 ----

pub(crate) fn h_equal(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = eval_compare(vm, "Equal", args[0], args[1], |o| o == Ordering::Equal);
    wrap_compare(vm, term, "Equal")
}

pub(crate) fn h_unequal(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = eval_compare(vm, "Unequal", args[0], args[1], |o| o != Ordering::Equal);
    wrap_compare(vm, term, "Unequal")
}

fn wrap_compare(vm: &mut Vm<'_>, term: TermId, head: &str) -> Outcome {
    if vm.head_name(term).is_some_and(|h| h == head) { Outcome::unevaluated(term) } else { Outcome::value(term) }
}

/// 列表广播 + 数值比较（legacy `eval_compare`）。
fn eval_compare(vm: &mut Vm<'_>, head: &str, left: TermId, right: TermId, cmp: fn(Ordering) -> bool) -> TermId {
    let l_list = matches!(vm.session.arena.get(left), Some(TermKind::List(_)));
    let r_list = matches!(vm.session.arena.get(right), Some(TermKind::List(_)));
    match (l_list, r_list) {
        (true, true) => {
            let (xs, ys) = (vm.app_args(left).unwrap_or_default(), vm.app_args(right).unwrap_or_default());
            if xs.len() != ys.len() {
                return vm.push_app(head, vec![left, right]);
            }
            let items = xs.iter().zip(ys.iter()).map(|(a, b)| eval_compare(vm, head, *a, *b, cmp)).collect();
            vm.push_list(items)
        }
        (true, false) => {
            let xs = vm.app_args(left).unwrap_or_default();
            let items = xs.iter().map(|a| eval_compare(vm, head, *a, right, cmp)).collect();
            vm.push_list(items)
        }
        (false, true) => {
            let ys = vm.app_args(right).unwrap_or_default();
            let items = ys.iter().map(|b| eval_compare(vm, head, left, *b, cmp)).collect();
            vm.push_list(items)
        }
        (false, false) => {
            if let Some(ord) = super::arith::num_compare_ids(vm, left, right) {
                return vm.push_bool(cmp(ord));
            }
            vm.push_app(head, vec![left, right])
        }
    }
}

// ---- 比较链（原始操作数：嵌套比较必须保持未求值）----

macro_rules! compare_chain_handler {
    ($name:ident, $op:expr, $pick:expr) => {
        pub(crate) fn $name(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
            compare_chain(vm, $op, args, $pick)
        }
    };
}

compare_chain_handler!(h_less_chain, "Less", |o: Ordering| o == Ordering::Less);
compare_chain_handler!(h_greater_chain, "Greater", |o: Ordering| o == Ordering::Greater);
compare_chain_handler!(h_less_equal_chain, "LessEqual", |o: Ordering| o != Ordering::Greater);
compare_chain_handler!(h_greater_equal_chain, "GreaterEqual", |o: Ordering| o != Ordering::Less);

const COMPARE_HEADS: [&str; 6] = ["Less", "Greater", "LessEqual", "GreaterEqual", "Equal", "Unequal"];

/// 左结合比较链：`Less[Less[1,2],3]` → `And[1<2, 2<3]`（legacy `eval_compare_chain`）。
fn compare_chain(vm: &mut Vm<'_>, op: &str, args: &[TermId], pick: fn(Ordering) -> bool) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app(op, args.to_vec()));
    }
    let nested = match vm.session.arena.get(args[0]) {
        Some(TermKind::App { args: inner, .. }) if inner.len() == 2 => {
            vm.head_name(args[0]).is_some_and(|h| COMPARE_HEADS.contains(&h.as_str()))
        }
        _ => false,
    };
    if nested {
        let left_o = vm.eval_value(args[0]);
        let inner = vm.app_args(args[0]).unwrap_or_default();
        let mid = inner.get(1).copied().unwrap_or(args[0]);
        let right_term = vm.push_app(op, vec![mid, args[1]]);
        let right_o = vm.eval_value(right_term);
        let mut diags = left_o.diagnostics.clone();
        diags.extend(right_o.diagnostics.clone());
        return match (as_boolean_id(vm, left_o.term), as_boolean_id(vm, right_o.term)) {
            (Some(a), Some(b)) => Outcome {
                term: vm.push_bool(a && b),
                kind: super::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: diags,
            },
            _ => {
                let term = vm.push_app("And", vec![left_o.term, right_o.term]);
                Outcome {
                    term,
                    kind: super::EvalKind::Unevaluated,
                    status: athena_types::ComputationStatus::Partial,
                    diagnostics: diags,
                }
            }
        };
    }
    let left_o = vm.eval_value(args[0]);
    let right_o = vm.eval_value(args[1]);
    let mut diags = left_o.diagnostics.clone();
    diags.extend(right_o.diagnostics.clone());
    let term = eval_compare(vm, op, left_o.term, right_o.term, pick);
    wrap_compare(vm, term, op).with_diagnostics(diags)
}

// ---- 逻辑 ----

pub(crate) fn h_and(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = logic_and(vm, args[0], args[1]);
    wrap_logic(vm, term, "And")
}

pub(crate) fn h_or(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = logic_or(vm, args[0], args[1]);
    wrap_logic(vm, term, "Or")
}

pub(crate) fn h_not(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = match as_boolean_id(vm, args[0]) {
        Some(v) => vm.push_bool(!v),
        None => vm.push_app("Not", vec![args[0]]),
    };
    wrap_logic(vm, term, "Not")
}

fn wrap_logic(vm: &mut Vm<'_>, term: TermId, head: &str) -> Outcome {
    if vm.head_name(term).is_some_and(|h| h == head) { Outcome::unevaluated(term) } else { Outcome::value(term) }
}

fn logic_and(vm: &mut Vm<'_>, left: TermId, right: TermId) -> TermId {
    match (as_boolean_id(vm, left), as_boolean_id(vm, right)) {
        (Some(a), Some(b)) => vm.push_bool(a && b),
        _ => vm.push_app("And", vec![left, right]),
    }
}

fn logic_or(vm: &mut Vm<'_>, left: TermId, right: TermId) -> TermId {
    match (as_boolean_id(vm, left), as_boolean_id(vm, right)) {
        (Some(a), Some(b)) => vm.push_bool(a || b),
        _ => vm.push_app("Or", vec![left, right]),
    }
}

// ---- 列表 / 一元数值 ----

pub(crate) fn h_list(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    Outcome::value(vm.push_list(args.to_vec()))
}

/// `Sin`/`Cos`/`Tan`/`Exp`/`Log`：精确三角值优先，其余保持符号（raw root 形式）。
pub(crate) fn h_unary_trig(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    let root = operands[0];
    let name = vm.head_name(root).unwrap_or_default();
    let args = vm.app_args(root).unwrap_or_default();
    if args.len() != 1 {
        let op = vm.session.operators.intern(&name);
        return Outcome::unevaluated(vm.rebuild_app_op(op, args));
    }
    let arg_o = vm.eval_value(args[0]);
    if arg_o.has_error() {
        return arg_o;
    }
    let term = if let Some(exact) = eval_trig_exact(vm, &name, arg_o.term) {
        exact
    }
    else if let Some(x) = term_as_f64(vm, arg_o.term) {
        let y = match name.as_str() {
            "Sin" => x.sin(),
            "Cos" => x.cos(),
            "Tan" => x.tan(),
            "Exp" => x.exp(),
            "Log" => x.ln(),
            _ => f64::NAN,
        };
        if y.is_finite() {
            super::arith::push_number(vm, athena_numeric::Number::machine(y))
        }
        else {
            vm.push_app(&name, vec![arg_o.term])
        }
    }
    else {
        vm.push_app(&name, vec![arg_o.term])
    };
    Outcome::value(term).with_diagnostics(arg_o.diagnostics)
}

/// 精确三角值（`Cos[Pi]`、`Sin[0]`、…；legacy `eval_trig_exact`）。
fn eval_trig_exact(vm: &mut Vm<'_>, name: &str, arg: TermId) -> Option<TermId> {
    let angle = normalize_pi_angle(vm, arg)?;
    match name {
        "Sin" => Some(vm.push_int(0)),
        "Cos" => Some(vm.push_int(if angle % 2 == 0 { 1 } else { -1 })),
        "Tan" if angle % 2 == 0 => Some(vm.push_int(0)),
        _ => None,
    }
}

/// 机器 `f64` 视图（`k·π` / `E` / 数字；legacy `term_as_f64`）。
pub(crate) fn term_as_f64(vm: &Vm<'_>, arg: TermId) -> Option<f64> {
    if let Some(k) = normalize_pi_angle(vm, arg) {
        return Some((k as f64) * std::f64::consts::PI);
    }
    if vm.head_name(arg).is_some_and(|h| h == "E") {
        return Some(std::f64::consts::E);
    }
    number_of(vm, arg).and_then(athena_numeric::to_f64_lossy)
}

/// `0`、`Pi`、`-Pi`、`n*Pi` → 整数 `n`（legacy `normalize_pi_angle`）。
pub(crate) fn normalize_pi_angle(vm: &Vm<'_>, arg: TermId) -> Option<i64> {
    if let Some(n) = number_of(vm, arg).and_then(|n| n.as_exact_integer()) {
        if n == 0 {
            return Some(0);
        }
    }
    if vm.head_name(arg).is_some_and(|h| h == "Pi") {
        return Some(1);
    }
    if is_app_named(vm, arg, "Times") {
        let args = vm.app_args(arg)?;
        if let [a, b] = args.as_slice() {
            if head_is(vm, *a, "Pi") {
                return number_of(vm, *b).and_then(|n| n.as_exact_integer());
            }
            if head_is(vm, *b, "Pi") {
                return number_of(vm, *a).and_then(|n| n.as_exact_integer());
            }
        }
        return None;
    }
    if is_app_named(vm, arg, "Plus") {
        let args = vm.app_args(arg)?;
        if args.len() == 1 && head_is(vm, args[0], "Pi") {
            return Some(1);
        }
    }
    None
}

fn is_app_named(vm: &Vm<'_>, id: TermId, name: &str) -> bool {
    vm.head_name(id).is_some_and(|h| h == name)
}

fn head_is(vm: &Vm<'_>, id: TermId, name: &str) -> bool {
    is_app_named(vm, id, name)
}

pub(crate) fn h_sqrt(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = if let Some(n) = number_of(vm, args[0]).map(|n| vm.copy_number(n).expect("sqrt copy")) {
        if let Ok(Some(v)) = athena_numeric::sqrt(&n) {
            super::arith::push_number(vm, v)
        }
        else {
            vm.push_app("Sqrt", vec![args[0]])
        }
    }
    else {
        vm.push_app("Sqrt", vec![args[0]])
    };
    Outcome::value(term)
}

pub(crate) fn h_abs(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = if let Some(n) = number_of(vm, args[0]).map(|n| vm.copy_number(n).expect("abs copy")) {
        super::arith::push_number(vm, athena_numeric::abs(n))
    }
    else {
        vm.push_app("Abs", vec![args[0]])
    };
    Outcome::value(term)
}

pub(crate) fn h_factorial(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = if let Some(n) = number_of(vm, args[0]).map(|n| vm.copy_number(n).expect("factorial copy")) {
        match athena_numeric::factorial(&n) {
            Ok(v) => super::arith::push_number(vm, v),
            Err(_) => vm.push_app("Factorial", vec![args[0]]),
        }
    }
    else {
        vm.push_app("Factorial", vec![args[0]])
    };
    Outcome::value(term)
}

pub(crate) fn h_simplify(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let e = vm.eval_value(args[0]);
    if e.has_error() {
        return e;
    }
    let term = if let Some(one) = try_pythagorean(vm, e.term) { one } else { vm.eval_value(e.term).term };
    Outcome::value(term).with_diagnostics(e.diagnostics)
}

/// `Sin[x]^2 + Cos[x]^2 → 1`（顺序可交换 · legacy `try_pythagorean`）。
fn try_pythagorean(vm: &mut Vm<'_>, expr: TermId) -> Option<TermId> {
    if !is_app_named(vm, expr, "Plus") {
        return None;
    }
    let terms = vm.app_args(expr)?;
    if terms.len() != 2 {
        return None;
    }
    let (a, b) = (terms[0], terms[1]);
    if is_trig_sq(vm, a, "Sin") && is_trig_sq(vm, b, "Cos") && same_trig_arg(vm, a, b) {
        return Some(vm.push_int(1));
    }
    if is_trig_sq(vm, a, "Cos") && is_trig_sq(vm, b, "Sin") && same_trig_arg(vm, a, b) {
        return Some(vm.push_int(1));
    }
    None
}

fn is_trig_sq(vm: &Vm<'_>, expr: TermId, name: &str) -> bool {
    let Some(TermKind::App { args, .. }) = vm.session.arena.get(expr)
    else {
        return false;
    };
    if args.len() != 2 || !is_app_named(vm, expr, "Power") {
        return false;
    }
    let exp_is_two =
        matches!(vm.session.arena.get(args[1]), Some(TermKind::Atom(AtomKind::Number(n))) if *n == Number::small_int(2));
    exp_is_two
        && matches!(vm.session.arena.get(args[0]), Some(TermKind::App { args: a, .. }) if a.len() == 1 && vm.head_name(args[0]).is_some_and(|h| h == name))
}

fn same_trig_arg(vm: &Vm<'_>, a: TermId, b: TermId) -> bool {
    fn arg(vm: &Vm<'_>, expr: TermId) -> Option<TermId> {
        let TermKind::App { args, .. } = vm.session.arena.get(expr)?
        else {
            return None;
        };
        if args.len() != 2 {
            return None;
        }
        let TermKind::App { args: inner, .. } = vm.session.arena.get(args[0])?
        else {
            return None;
        };
        if inner.len() == 1 { Some(inner[0]) } else { None }
    }
    match (arg(vm, a), arg(vm, b)) {
        (Some(x), Some(y)) => vm.session.arena.structural_eq(x, y),
        _ => false,
    }
}

// ---- Range / Length / First / Join ----

pub(crate) fn h_range(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let mut ints = Vec::with_capacity(args.len());
    for a in args {
        match number_of(vm, *a).and_then(|n| n.as_exact_integer()) {
            Some(v) => ints.push(v),
            None => return Outcome::unevaluated(vm.push_app("Range", args.to_vec())),
        }
    }
    let list = match ints.as_slice() {
        [n] => range_ints(vm, 1, *n, 1),
        [a, b] => range_ints(vm, *a, *b, 1),
        [a, b, step] => range_ints(vm, *a, *b, *step),
        _ => None,
    };
    match list {
        Some(items) => Outcome::value(vm.push_list(items)),
        None => Outcome::unevaluated(vm.push_app("Range", args.to_vec())),
    }
}

pub(crate) fn range_ints(vm: &mut Vm<'_>, a: i64, b: i64, step: i64) -> Option<Vec<TermId>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut x = a;
    if step > 0 {
        while x <= b {
            out.push(vm.push_int(x));
            x += step;
        }
    }
    else {
        while x >= b {
            out.push(vm.push_int(x));
            x += step;
        }
    }
    Some(out)
}

pub(crate) fn h_length(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let term = match vm.app_args(args[0]) {
        Some(items) => vm.push_int(items.len() as i64),
        None => return Outcome::unevaluated(vm.push_app("Length", vec![args[0]])),
    };
    Outcome::value(term)
}

pub(crate) fn h_first(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    match vm.app_args(args[0]) {
        Some(items) if !items.is_empty() => Outcome::value(items[0]),
        Some(items) => {
            let echo = vm.push_app("First", vec![args[0]]);
            Outcome::invalid(echo, invalid_index_diagnostic(1, Some(items.len() as u64)))
        }
        None => Outcome::unevaluated(vm.push_app("First", vec![args[0]])),
    }
}

pub(crate) fn h_join(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let mut out = Vec::new();
    for arg in args {
        match vm.app_args(*arg) {
            Some(items) if matches!(vm.session.arena.get(*arg), Some(TermKind::List(_))) => out.extend(items),
            _ => return Outcome::unevaluated(vm.push_app("Join", args.to_vec())),
        }
    }
    Outcome::value(vm.push_list(out))
}

// ---- 诊断捷径 ----

pub(crate) fn h_unsupported(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    let root = operands[0];
    let name = vm.head_name(root).unwrap_or_default();
    Outcome::invalid(root, Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", name))
}

pub(crate) fn h_error(vm: &mut Vm<'_>, operands: &[TermId]) -> Outcome {
    let root = operands[0];
    let name = vm.head_name(root).unwrap_or_default();
    let args = vm.app_args(root).unwrap_or_default();
    let mut evaluated = Vec::with_capacity(args.len());
    let mut diags = Vec::new();
    for a in args {
        let o = vm.eval_value(a);
        diags.extend(o.diagnostics);
        evaluated.push(o.term);
    }
    let op = vm.session.operators.intern(&name);
    let echo = vm.rebuild_app_op(op, evaluated);
    let msg = match vm.session.arena.get(vm.app_args(echo).unwrap_or_default().first().copied().unwrap_or(echo)) {
        Some(TermKind::Atom(AtomKind::String(s))) => s.clone(),
        _ => "error".to_string(),
    };
    Outcome::invalid(
        echo,
        Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("operation", "error").detail("message", msg),
    )
    .with_diagnostics(diags)
}

/// 值位 `Set` / `SetDelayed` quirk：参数已求值，再求值 rhs 一次（legacy 双重求值一致）。
pub(crate) fn h_set_eval_rhs(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    vm.eval_value(args[1])
}
