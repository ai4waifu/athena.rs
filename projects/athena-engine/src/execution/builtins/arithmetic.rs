//! Plus / Times / Power 算术规范化 handler（legacy `eval_plus` / `eval_times` / `eval_power` 语义）。

use athena_ir::{Atom, TermNode};
use athena_numeric::{Number, add as num_add, compare as num_compare, div as num_div, mul as num_mul, pow as num_pow};
use athena_types::TermId;

use crate::execution::{
    TermEvaluation,
    vm::{Shape, Vm},
};

/// 向 arena 压入数字原子。
pub(crate) fn push_number(vm: &mut Vm<'_>, n: Number) -> TermId {
    let span = TermNode::default_span();
    vm.session.arena.push(TermNode::Atom(Atom::Number(n)), span)
}

/// 读 arena 数字引用。
pub(crate) fn number_of<'a>(vm: &'a Vm<'_>, id: TermId) -> Option<&'a Number> {
    match vm.session.arena.get(id) {
        Some(TermNode::Atom(Atom::Number(n))) => Some(n),
        _ => None,
    }
}

fn is_application_named(vm: &Vm<'_>, id: TermId, name: &str) -> bool {
    vm.head_name(id).is_some_and(|h| h == name)
}

pub(crate) fn h_plus(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    TermEvaluation::value(plus(vm, args))
}

pub(crate) fn plus(vm: &mut Vm<'_>, args: &[TermId]) -> TermId {
    let mut flat: Vec<TermId> = Vec::new();
    let mut sum: Option<Number> = None;
    for a in args {
        flatten_plus(vm, *a, &mut flat, &mut sum);
    }
    if let Some(s) = sum {
        if !s.is_zero() {
            flat.insert(0, push_number(vm, s));
        }
    }
    else if flat.is_empty() {
        return vm.push_int(0);
    }
    let flat = combine_like_plus_terms(vm, flat);
    match flat.len() {
        0 => vm.push_int(0),
        1 => flat[0],
        _ => vm.push_application("Plus", flat),
    }
}

/// 合并 `c1·k + c2·k`（裸 `k` 视为系数 1）。
fn combine_like_plus_terms(vm: &mut Vm<'_>, terms: Vec<TermId>) -> Vec<TermId> {
    let mut groups: Vec<(TermId, Number)> = Vec::new();
    for t in terms {
        let (coef, kernel) = split_numeric_coeff(vm, t);
        let mut matched = false;
        for (k, acc) in groups.iter_mut() {
            if vm.session.arena.structural_eq(*k, kernel) {
                match num_add(vm.copy_number(acc).expect("group coeff copy"), vm.copy_number(&coef).expect("coeff copy")) {
                    Ok(v) => *acc = v,
                    Err(_) => return groups_to_plus_terms(vm, groups), // 回退：放弃合并
                }
                matched = true;
                break;
            }
        }
        if !matched {
            groups.push((kernel, coef));
        }
    }
    groups_to_plus_terms(vm, groups)
}

fn groups_to_plus_terms(vm: &mut Vm<'_>, groups: Vec<(TermId, Number)>) -> Vec<TermId> {
    let mut out = Vec::new();
    for (kernel, coef) in groups {
        if coef.is_zero() {
            continue;
        }
        else if number_of(vm, kernel).is_some_and(Number::is_one) {
            out.push(push_number(vm, coef));
        }
        else if coef.is_one() {
            out.push(kernel);
        }
        else {
            let coef_id = push_number(vm, coef);
            out.push(times(vm, &[coef_id, kernel]));
        }
    }
    out
}

/// 拆出数值系数：`Times[c, rest…]` / 纯数 / 其它。
fn split_numeric_coeff(vm: &mut Vm<'_>, term: TermId) -> (Number, TermId) {
    if let Some(Shape::Application(op, args)) = vm.shape(term) {
        if vm.session.operators.name(op) == Some("Times") && !args.is_empty() {
            let mut coef = Number::small_int(1);
            let mut rest = Vec::new();
            for a in args {
                if let Some(n) = number_of(vm, a) {
                    let n = vm.copy_number(n).expect("coeff copy");
                    coef = num_mul(vm.copy_number(&coef).expect("coef copy"), n).unwrap_or(coef);
                }
                else {
                    rest.push(a);
                }
            }
            let kernel = match rest.len() {
                0 => vm.push_int(1),
                1 => rest[0],
                _ => vm.push_application("Times", rest),
            };
            return (coef, kernel);
        }
    }
    if let Some(n) = number_of(vm, term) {
        let n = vm.copy_number(n).expect("number copy");
        return (n, vm.push_int(1));
    }
    (Number::small_int(1), term)
}

fn flatten_plus(vm: &mut Vm<'_>, a: TermId, flat: &mut Vec<TermId>, sum: &mut Option<Number>) {
    if is_application_named(vm, a, "Plus") {
        let args = vm.application_arguments(a).unwrap_or_default();
        for x in args {
            flatten_plus(vm, x, flat, sum);
        }
        return;
    }
    push_plus_term(vm, a, flat, sum);
}

fn push_plus_term(vm: &mut Vm<'_>, a: TermId, flat: &mut Vec<TermId>, sum: &mut Option<Number>) {
    if let Some(n) = number_of(vm, a) {
        let n = vm.copy_number(n).expect("summand copy");
        *sum = Some(match sum.take() {
            Some(s) => num_add(vm.copy_number(&s).expect("sum copy"), n).unwrap_or(s),
            None => n,
        });
    }
    else {
        flat.push(a);
    }
}

pub(crate) fn h_times(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    times_outcome(vm, args)
}

/// `Times`：标量 × nested List 按 MATLAB/数组语义广播；两矩阵走 `matmul`；其它保持符号 `Times`。
fn times_outcome(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    let mut scalars = Vec::new();
    let mut lists = Vec::new();
    let mut other = false;
    for a in args {
        if number_of(vm, *a).is_some() {
            scalars.push(*a);
        }
        else if matches!(vm.session.arena.get(*a), Some(TermNode::List(_))) {
            lists.push(*a);
        }
        else {
            other = true;
            break;
        }
    }
    if !other && lists.len() == 1 && !scalars.is_empty() && scalars.len() + 1 == args.len() {
        let scale = times(vm, &scalars);
        return super::matrix::dot_binop(vm, "DotTimes", scale, lists[0], super::matrix::DotOpKind::Times);
    }
    if !other && lists.len() == 2 && scalars.is_empty() && args.len() == 2 {
        let am = super::matrix::term_to_rational_matrix(vm, lists[0]);
        let bm = super::matrix::term_to_rational_matrix(vm, lists[1]);
        if let (Some(am), Some(bm)) = (am, bm) {
            let echo = vm.push_application("Times", args.to_vec());
            return match crate::domains::linear_algebra::matmul(&am, &bm) {
                Ok(m) => match super::matrix::matrix_to_nested_list(vm, &m) {
                    Ok(term) => TermEvaluation::value(term),
                    Err(d) => TermEvaluation::invalid(echo, d),
                },
                Err(d) => TermEvaluation::invalid(echo, d),
            };
        }
    }
    TermEvaluation::value(times(vm, args))
}

pub(crate) fn times(vm: &mut Vm<'_>, args: &[TermId]) -> TermId {
    let mut flat: Vec<TermId> = Vec::new();
    let mut prod: Option<Number> = None;
    for a in args {
        flatten_times(vm, *a, &mut flat, &mut prod);
        if prod.as_ref().is_some_and(Number::is_zero) {
            return vm.push_int(0);
        }
    }
    if let Some(p) = prod {
        if !p.is_one() {
            flat.insert(0, push_number(vm, p));
        }
    }
    else if flat.is_empty() {
        return vm.push_int(1);
    }
    // Times 对 Plus 分配：c·(a+b) → c·a + c·b（仅单层，避免爆炸）
    if let Some(idx) = flat.iter().position(|t| is_application_named(vm, *t, "Plus")) {
        let plus_id = flat.remove(idx);
        if let Some(summands) = vm.application_arguments(plus_id) {
            let mut parts = Vec::with_capacity(summands.len());
            for s in summands {
                let mut factors = flat.clone();
                factors.push(s);
                parts.push(times(vm, &factors));
            }
            return plus(vm, &parts);
        }
    }
    let merged = combine_like_powers(vm, flat);
    let flat = canonicalize_times_factors(vm, merged);
    match flat.len() {
        0 => vm.push_int(1),
        1 => flat[0],
        _ => vm.push_application("Times", flat),
    }
}

/// 数值系数前置，保持与历史 Times 规范一致。
fn canonicalize_times_factors(vm: &mut Vm<'_>, factors: Vec<TermId>) -> Vec<TermId> {
    let mut nums = Vec::new();
    let mut rest = Vec::new();
    for f in factors {
        if number_of(vm, f).is_some() {
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
fn combine_like_powers(vm: &mut Vm<'_>, factors: Vec<TermId>) -> Vec<TermId> {
    let mut out: Vec<(TermId, TermId)> = Vec::new(); // (base, exp)
    let mut rest = Vec::new();
    for f in factors {
        let base_exp = match vm.shape(f) {
            Some(Shape::Application(op, args)) if vm.session.operators.name(op) == Some("Power") && args.len() == 2 => Some((args[0], args[1])),
            Some(Shape::Symbol(_)) => {
                let one = vm.push_int(1);
                Some((f, one))
            }
            _ => None,
        };
        match base_exp {
            Some((base, exp)) => {
                let mut merged = false;
                for (b, e) in out.iter_mut() {
                    if vm.session.arena.structural_eq(*b, base) {
                        let combined = plus(vm, &[*e, exp]);
                        *e = combined;
                        merged = true;
                        break;
                    }
                }
                if !merged {
                    out.push((base, exp));
                }
            }
            None => rest.push(f),
        }
    }
    let mut merged: Vec<TermId> = Vec::new();
    for (base, exp) in out {
        let p = power(vm, base, exp);
        if number_of(vm, p).is_some_and(Number::is_one) {
            continue;
        }
        merged.push(p);
    }
    merged.extend(rest);
    merged
}

fn flatten_times(vm: &mut Vm<'_>, a: TermId, flat: &mut Vec<TermId>, prod: &mut Option<Number>) {
    if is_application_named(vm, a, "Times") {
        let args = vm.application_arguments(a).unwrap_or_default();
        for x in args {
            flatten_times(vm, x, flat, prod);
        }
        return;
    }
    if let Some(n) = number_of(vm, a) {
        let n = vm.copy_number(n).expect("factor copy");
        if n.is_zero() {
            *prod = Some(Number::small_int(0));
            return;
        }
        *prod = Some(match prod.take() {
            Some(p) => num_mul(vm.copy_number(&p).expect("prod copy"), n).unwrap_or(p),
            None => n,
        });
    }
    else {
        flat.push(a);
    }
}

pub(crate) fn h_power(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    TermEvaluation::value(power(vm, args[0], args[1]))
}

pub(crate) fn power(vm: &mut Vm<'_>, base: TermId, exp: TermId) -> TermId {
    if let Some(e) = number_of(vm, exp).map(|n| vm.copy_number(n).expect("exp copy")) {
        if e.is_zero() {
            // 标量 `x^0 → 1`；List 用 `DotPower` 逐元素，此路不处理。
            if matches!(vm.shape(base), Some(Shape::List(_))) {
                return vm.push_application("Power", vec![base, exp]);
            }
            return vm.push_int(1);
        }
        if e.is_one() {
            return base;
        }
        if e.is_neg_one() {
            if let Some(b) = number_of(vm, base).map(|n| vm.copy_number(n).expect("base copy")) {
                if let Ok(v) = num_div(Number::small_int(1), b) {
                    return push_number(vm, v);
                }
            }
        }
        // (c * u)^n → c^n * u^n（整数 n）；(u^a)^b → u^(a*b)（a,b 为数）
        if e.as_integer_exp().is_some() {
            if let Some(Shape::Application(op, args)) = vm.shape(base) {
                if vm.session.operators.name(op) == Some("Times") && args.len() >= 2 {
                    if let Some(c) = number_of(vm, args[0]).map(|n| vm.copy_number(n).expect("coef copy")) {
                        if let Ok(cp) = num_pow(&c, &e) {
                            let rest = if args.len() == 2 { args[1] } else { vm.push_application("Times", args[1..].to_vec()) };
                            let rest_pow = power(vm, rest, exp);
                            let cp_id = push_number(vm, cp);
                            return times(vm, &[cp_id, rest_pow]);
                        }
                    }
                }
                if vm.session.operators.name(op) == Some("Power") && args.len() == 2 {
                    if let Some(a) = number_of(vm, args[1]).map(|n| vm.copy_number(n).expect("inner exp copy")) {
                        if let Ok(ab) = num_mul(a, vm.copy_number(&e).expect("e copy")) {
                            let ab_id = push_number(vm, ab);
                            return power(vm, args[0], ab_id);
                        }
                    }
                }
            }
        }
    }
    if let (Some(b), Some(e)) =
        (number_of(vm, base).map(|n| vm.copy_number(n).expect("b copy")), number_of(vm, exp).map(|n| vm.copy_number(n).expect("e copy")))
    {
        if let Ok(v) = num_pow(&b, &e) {
            return push_number(vm, v);
        }
    }
    vm.push_application("Power", vec![base, exp])
}

pub(crate) fn h_subtract(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    let one = vm.push_int(-1);
    let neg = times(vm, &[one, args[1]]);
    TermEvaluation::value(plus(vm, &[args[0], neg]))
}

pub(crate) fn h_divide(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    let one = vm.push_int(-1);
    let inv = power(vm, args[1], one);
    TermEvaluation::value(times(vm, &[args[0], inv]))
}

pub(crate) fn h_dot_times(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    super::matrix::dot_binop(vm, "DotTimes", args[0], args[1], super::matrix::DotOpKind::Times)
}

pub(crate) fn h_dot_divide(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    super::matrix::dot_binop(vm, "DotDivide", args[0], args[1], super::matrix::DotOpKind::Divide)
}

pub(crate) fn h_dot_power(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    super::matrix::dot_binop(vm, "DotPower", args[0], args[1], super::matrix::DotOpKind::Power)
}

pub(crate) fn h_mldivide(vm: &mut Vm<'_>, operands: &[TermId]) -> TermEvaluation {
    let root = operands[0];
    let name = vm.head_name(root).unwrap_or_default();
    let args = vm.application_arguments(root).unwrap_or_default();
    if args.len() != 2 {
        return TermEvaluation::invalid(
            root,
            athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation).detail("operation", name),
        );
    }
    let a = vm.eval_value(args[0]);
    let b = vm.eval_value(args[1]);
    let diags = {
        let mut d = a.diagnostics.clone();
        d.extend(b.diagnostics.clone());
        d
    };
    let _ = &diags;
    let op = vm.session.operators.intern(&name);
    let echo = vm.rebuild_application_operator(op, vec![a.term, b.term]);
    if a.has_error() || b.has_error() {
        let mut d = a.diagnostics.clone();
        d.extend(b.diagnostics.clone());
        return TermEvaluation {
            term: echo,
            kind: crate::execution::EvalKind::Unevaluated,
            status: athena_types::ComputationStatus::Invalid,
            diagnostics: d,
        };
    }
    super::matrix::mldivide(vm, &name, a.term, b.term, echo).with_diagnostics(diags)
}

/// 数值比较（供 builtin / lin 使用）。
pub(crate) fn num_compare_ids(vm: &Vm<'_>, a: TermId, b: TermId) -> Option<std::cmp::Ordering> {
    let (na, nb) = (number_of(vm, a)?, number_of(vm, b)?);
    num_compare(na, nb)
}
