//! 引擎桥接 [`Term`] 上的内建求值。

use std::cmp::Ordering;

use num_traits::ToPrimitive;

use athena_types::{Number, Result};

pub use crate::calculus::differentiate;

use crate::term::{Atom, Term, number_from_term};

fn map_num<T>(r: Result<T>) -> Result<T> {
    r
}

/// 在内建定义下求值表达式。未知头部保留为 Application。
pub fn evaluate(expr: &Term) -> Term {
    evaluate_depth(expr, 0)
}

fn evaluate_depth(expr: &Term, depth: u32) -> Term {
    if depth > 256 {
        return expr.clone();
    }
    match expr {
        Term::Atom(_) => expr.clone(),
        Term::List(items) => Term::List(items.iter().map(|i| evaluate_depth(i, depth + 1)).collect()),
        Term::Application { head, arguments: args } => {
            let head_e = evaluate_depth(head, depth + 1);
            let args_e: Vec<Term> = args.iter().map(|a| evaluate_depth(a, depth + 1)).collect();
            apply_builtin(&head_e, args_e, depth)
        }
    }
}

fn apply_builtin(head: &Term, args: Vec<Term>, depth: u32) -> Term {
    let name = match head {
        Term::Atom(Atom::Symbol(s)) => s.as_str(),
        _ => {
            return Term::Application { head: Box::new(head.clone()), arguments: args };
        }
    };

    match name {
        "Plus" => eval_plus(args),
        "Times" => eval_times(args),
        "Power" if args.len() == 2 => eval_power(args[0].clone(), args[1].clone()),
        "Subtract" if args.len() == 2 => eval_plus(vec![args[0].clone(), eval_times(vec![Term::int(-1), args[1].clone()])]),
        "Divide" if args.len() == 2 => eval_times(vec![args[0].clone(), eval_power(args[1].clone(), Term::int(-1))]),
        "List" => Term::List(args),
        "Simplify" if args.len() == 1 => eval_simplify(&args[0], depth),
        "Sin" | "Cos" | "Tan" | "Exp" | "Log" if args.len() == 1 => {
            Term::Application { head: Box::new(head.clone()), arguments: args }
        }
        "Sqrt" if args.len() == 1 => eval_sqrt(&args[0]),
        "Abs" if args.len() == 1 => eval_abs(&args[0]),
        "Factorial" if args.len() == 1 => eval_factorial(&args[0]),
        "Map" if args.len() == 2 => eval_map(&args[0], &args[1], depth),
        "Equal" if args.len() == 2 => eval_compare("Equal", &args[0], &args[1], |o| o == Ordering::Equal),
        "Unequal" if args.len() == 2 => eval_compare("Unequal", &args[0], &args[1], |o| o != Ordering::Equal),
        "Less" if args.len() == 2 => eval_compare("Less", &args[0], &args[1], |o| o == Ordering::Less),
        "Greater" if args.len() == 2 => eval_compare("Greater", &args[0], &args[1], |o| o == Ordering::Greater),
        "LessEqual" if args.len() == 2 => eval_compare("LessEqual", &args[0], &args[1], |o| o != Ordering::Greater),
        "GreaterEqual" if args.len() == 2 => eval_compare("GreaterEqual", &args[0], &args[1], |o| o != Ordering::Less),
        "And" if args.len() == 2 => eval_logic_and(&args[0], &args[1]),
        "Or" if args.len() == 2 => eval_logic_or(&args[0], &args[1]),
        "Not" if args.len() == 1 => eval_logic_not(&args[0]),
        "Set" | "SetDelayed" if args.len() == 2 => evaluate_depth(&args[1], depth + 1),
        "D" | "Integrate" | "Limit" | "Series" | "DSolve" | "LaplaceTransform" => {
            let term = Term::Application { head: Box::new(head.clone()), arguments: args };
            if let Some(req) = crate::calculus::try_calculus_request(&term) {
                let result = crate::calculus::execute_calculus(req);
                return crate::calculus::calculus_result_bridge_term(&result);
            }
            term
        }
        "CompoundExpression" if !args.is_empty() => evaluate_depth(args.last().unwrap(), depth + 1),
        "Function" => Term::Application { head: Box::new(Term::symbol("Function")), arguments: args },
        "ReplaceAll" if args.len() == 2 => eval_replace_all(&args[0], &args[1], depth),
        "Part" if args.len() == 2 => eval_part(&args[0], &args[1]),
        "Rule" | "RuleDelayed" if args.len() == 2 => Term::Application { head: Box::new(head.clone()), arguments: args },
        _ => {
            if let Term::Application { head: fh, arguments: fargs } = head {
                if fh.is_symbol("Function") && fargs.len() == 1 && args.len() == 1 {
                    let body = substitute_slot(&fargs[0], &args[0]);
                    return evaluate_depth(&body, depth + 1);
                }
            }
            Term::Application { head: Box::new(head.clone()), arguments: args }
        }
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
    match flat.len() {
        0 => Term::int(0),
        1 => flat.pop().unwrap(),
        _ => Term::apply("Plus", flat),
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
    if let Some(n) = number_from_term(&a).cloned() {
        *sum = Some(match sum.take() {
            Some(s) => map_num(s.clone().add(n)).unwrap_or(s),
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
    match flat.len() {
        0 => Term::int(1),
        1 => flat.pop().unwrap(),
        _ => Term::apply("Times", flat),
    }
}

fn flatten_times(a: Term, flat: &mut Vec<Term>, prod: &mut Option<Number>) {
    match a {
        Term::Application { head, arguments: args } if head.is_symbol("Times") => {
            for x in args {
                flatten_times(x, flat, prod);
            }
        }
        other => {
            if let Some(n) = number_from_term(&other).cloned() {
                if n.is_zero() {
                    *prod = Some(Number::small_int(0));
                    return;
                }
                *prod = Some(match prod.take() {
                    Some(p) => map_num(p.clone().mul(n)).unwrap_or(p),
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
    if let Some(e) = number_from_term(&exp).cloned() {
        if e.is_zero() {
            return Term::int(1);
        }
        if e.is_one() {
            return base;
        }
        if e.is_neg_one() {
            if let Some(b) = number_from_term(&base).cloned() {
                if let Ok(v) = map_num(Number::small_int(1).div(b)) {
                    return Term::number(v);
                }
            }
        }
    }
    if let (Some(b), Some(e)) = (number_from_term(&base).cloned(), number_from_term(&exp).cloned()) {
        if let Ok(v) = map_num(b.pow(&e)) {
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

fn eval_sqrt(arg: &Term) -> Term {
    if let Some(n) = number_from_term(arg).cloned() {
        if let Ok(Some(v)) = map_num(n.sqrt()) {
            return Term::number(v);
        }
    }
    Term::apply("Sqrt", vec![arg.clone()])
}

fn eval_abs(arg: &Term) -> Term {
    if let Some(n) = number_from_term(arg).cloned() {
        return Term::number(n.abs());
    }
    Term::apply("Abs", vec![arg.clone()])
}

fn eval_factorial(arg: &Term) -> Term {
    if let Some(n) = number_from_term(arg).cloned() {
        if let Ok(v) = map_num(n.factorial()) {
            return Term::number(v);
        }
    }
    Term::apply("Factorial", vec![arg.clone()])
}

fn eval_compare<F>(head: &str, left: &Term, right: &Term, cmp: F) -> Term
where
    F: Fn(Ordering) -> bool,
{
    if let (Some(a), Some(b)) = (number_from_term(left), number_from_term(right)) {
        if let Some(ord) = a.compare(b) {
            return Term::int(if cmp(ord) { 1 } else { 0 });
        }
    }
    Term::apply(head, vec![left.clone(), right.clone()])
}

fn eval_logic_and(left: &Term, right: &Term) -> Term {
    match (truthy(left), truthy(right)) {
        (Some(a), Some(b)) => Term::int(if a && b { 1 } else { 0 }),
        _ => Term::apply("And", vec![left.clone(), right.clone()]),
    }
}

fn eval_logic_or(left: &Term, right: &Term) -> Term {
    match (truthy(left), truthy(right)) {
        (Some(a), Some(b)) => Term::int(if a || b { 1 } else { 0 }),
        _ => Term::apply("Or", vec![left.clone(), right.clone()]),
    }
}

fn eval_logic_not(arg: &Term) -> Term {
    match truthy(arg) {
        Some(v) => Term::int(if v { 0 } else { 1 }),
        None => Term::apply("Not", vec![arg.clone()]),
    }
}

fn truthy(expr: &Term) -> Option<bool> {
    number_from_term(expr).map(Number::is_truthy)
}

fn eval_map(func: &Term, target: &Term, depth: u32) -> Term {
    let list = match target {
        Term::List(items) => items,
        other => return Term::apply("Map", vec![func.clone(), other.clone()]),
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
        Term::Atom(Atom::Symbol(name)) => Term::apply(name.clone(), vec![item.clone()]),
        Term::Application { head, arguments: args } if head.is_symbol("Function") && args.len() == 1 => {
            substitute_slot(&args[0], item)
        }
        _ => Term::apply("Map", vec![func.clone(), item.clone()]),
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
    let mut cur = expr.clone();
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
            Some((args[0].clone(), args[1].clone()))
        }
        _ => None,
    }
}

fn replace_literal(expr: &Term, lhs: &Term, rhs: &Term) -> Term {
    if expr == lhs {
        return rhs.clone();
    }
    match expr {
        Term::List(items) => Term::List(items.iter().map(|i| replace_literal(i, lhs, rhs)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(replace_literal(head, lhs, rhs)),
            arguments: args.iter().map(|a| replace_literal(a, lhs, rhs)).collect(),
        },
        other => other.clone(),
    }
}

fn eval_part(expr: &Term, index: &Term) -> Term {
    let idx = match number_from_term(index).and_then(|n| n.as_exact_integer()) {
        Some(n) => match n.to_i64() {
            Some(v) => v,
            None => return Term::apply("Part", vec![expr.clone(), index.clone()]),
        },
        None => return Term::apply("Part", vec![expr.clone(), index.clone()]),
    };
    match expr {
        Term::List(items) => {
            let i = if idx > 0 {
                (idx - 1) as usize
            }
            else if idx < 0 {
                items.len().wrapping_add(idx as usize)
            }
            else {
                return Term::apply("Part", vec![expr.clone(), index.clone()]);
            };
            items.get(i).cloned().unwrap_or_else(|| Term::apply("Part", vec![expr.clone(), index.clone()]))
        }
        _ => Term::apply("Part", vec![expr.clone(), index.clone()]),
    }
}

fn substitute_slot(body: &Term, value: &Term) -> Term {
    match body {
        Term::Atom(Atom::Symbol(s)) if s == "#" || s == "#1" => value.clone(),
        Term::Atom(_) => body.clone(),
        Term::List(items) => Term::List(items.iter().map(|i| substitute_slot(i, value)).collect()),
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(substitute_slot(head, value)),
            arguments: args.iter().map(|a| substitute_slot(a, value)).collect(),
        },
    }
}
