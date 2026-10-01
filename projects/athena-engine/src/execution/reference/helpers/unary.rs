//! 一元结构 / 数值算子求值（Reference 与 `ExecutionHost` 共用）。

use athena_ir::SemanticOperator;
use athena_numeric::{abs as num_abs, factorial as num_factorial, sqrt as num_sqrt, Integer, Number, Rational, Real};
use athena_types::{Result, TermId};

use super::diag;
use crate::{
    execution::{number_of, push_extension, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{
            arena::{push_int, push_list, push_symbol_name},
            numeric_clone::clone_number,
        },
    },
};

/// 一元 `Abs` / `Factorial` / `Sqrt` / `Floor` / `Length` / `First` / `Rest` / `Most` / `Reverse` / `Flatten` / `Head`。
pub(crate) fn evaluate_unary_term(session: &mut Session, op: SemanticOperator, term: TermId) -> Result<TermId> {
    match op {
        SemanticOperator::Abs => {
            if let Some(n) = number_of(session, term) {
                Ok(push_number(session, num_abs(clone_number(n))))
            }
            else {
                Ok(push_semantic(session, SemanticOperator::Abs, vec![term]))
            }
        }
        SemanticOperator::Floor => {
            if let Some(n) = number_of(session, term) {
                if let Some(floored) = exact_floor(&clone_number(n)) {
                    return Ok(push_number(session, floored));
                }
            }
            Ok(push_semantic(session, SemanticOperator::Floor, vec![term]))
        }
        SemanticOperator::Ceiling => {
            if let Some(n) = number_of(session, term) {
                if let Some(ceiled) = exact_ceiling(&clone_number(n)) {
                    return Ok(push_number(session, ceiled));
                }
            }
            Ok(push_semantic(session, SemanticOperator::Ceiling, vec![term]))
        }
        SemanticOperator::Round => {
            if let Some(n) = number_of(session, term) {
                if let Some(rounded) = exact_round(&clone_number(n)) {
                    return Ok(push_number(session, rounded));
                }
            }
            Ok(push_semantic(session, SemanticOperator::Round, vec![term]))
        }
        SemanticOperator::Numerator => {
            if let Some(n) = number_of(session, term) {
                if let Some(num) = exact_numerator(&clone_number(n)) {
                    return Ok(push_number(session, num));
                }
            }
            Ok(push_semantic(session, SemanticOperator::Numerator, vec![term]))
        }
        SemanticOperator::Denominator => {
            if let Some(n) = number_of(session, term) {
                if let Some(denom) = exact_denominator(&clone_number(n)) {
                    return Ok(push_number(session, denom));
                }
            }
            Ok(push_semantic(session, SemanticOperator::Denominator, vec![term]))
        }
        SemanticOperator::UnitStep => {
            if let Some(n) = number_of(session, term) {
                if let Some(step) = exact_unit_step(&clone_number(n)) {
                    return Ok(push_number(session, step));
                }
            }
            Ok(push_semantic(session, SemanticOperator::UnitStep, vec![term]))
        }
        SemanticOperator::Factorial => {
            if let Some(n) = number_of(session, term) {
                match num_factorial(n) {
                    Ok(v) => Ok(push_number(session, v)),
                    Err(_) => Ok(push_semantic(session, SemanticOperator::Factorial, vec![term])),
                }
            }
            else {
                Ok(push_semantic(session, SemanticOperator::Factorial, vec![term]))
            }
        }
        SemanticOperator::Sqrt => {
            if let Some(n) = number_of(session, term) {
                match num_sqrt(n) {
                    Ok(Some(v)) => Ok(push_number(session, v)),
                    _ => Ok(push_semantic(session, SemanticOperator::Sqrt, vec![term])),
                }
            }
            else {
                Ok(push_semantic(session, SemanticOperator::Sqrt, vec![term]))
            }
        }
        SemanticOperator::IntegerDigits => {
            if let Some(n) = number_of(session, term) {
                if let Some(i) = n.as_exact_integer() {
                    if i == 0 {
                        return Ok(push_list(session, vec![]));
                    }
                    let negative = i < 0;
                    let mut v = i.abs();
                    let mut digits = Vec::new();
                    while v > 0 {
                        digits.push(v % 10);
                        v /= 10;
                    }
                    digits.reverse();
                    if negative {
                        digits[0] = -digits[0];
                    }
                    let items = digits.into_iter().map(|d| push_int(session, d)).collect();
                    return Ok(push_list(session, items));
                }
            }
            Ok(push_semantic(session, SemanticOperator::IntegerDigits, vec![term]))
        }
        SemanticOperator::Length => {
            let len = match session.arena.get(term) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.len() as i64,
                Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.len() as i64,
                _ => return Ok(push_semantic(session, SemanticOperator::Length, vec![term])),
            };
            Ok(session.builder().int(len, Default::default()))
        }
        SemanticOperator::First => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => Ok(items[0]),
            Some(athena_ir::TermNode::Application { arguments, .. }) if !arguments.is_empty() => Ok(arguments[0]),
            Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("first_empty")),
            _ => Ok(push_semantic(session, SemanticOperator::First, vec![term])),
        },
        SemanticOperator::Rest => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => {
                let rest = items[1..].to_vec();
                Ok(push_list(session, rest))
            }
            Some(athena_ir::TermNode::Application { head, arguments }) if !arguments.is_empty() => {
                let head = *head;
                let rest = arguments[1..].to_vec();
                Ok(session.builder().application(head, rest, Default::default()))
            }
            Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("rest_empty")),
            _ => Ok(push_semantic(session, SemanticOperator::Rest, vec![term])),
        },
        SemanticOperator::Most => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => {
                let most = items[..items.len() - 1].to_vec();
                Ok(push_list(session, most))
            }
            Some(athena_ir::TermNode::Application { head, arguments }) if !arguments.is_empty() => {
                let head = *head;
                let most = arguments[..arguments.len() - 1].to_vec();
                Ok(session.builder().application(head, most, Default::default()))
            }
            Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("most_empty")),
            _ => Ok(push_semantic(session, SemanticOperator::Most, vec![term])),
        },
        SemanticOperator::Reverse => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
                let mut rev = items.clone();
                rev.reverse();
                Ok(push_list(session, rev))
            }
            Some(athena_ir::TermNode::Application { head, arguments }) if arguments.len() > 1 => {
                let head = *head;
                let mut rev = arguments.clone();
                rev.reverse();
                Ok(session.builder().application(head, rev, Default::default()))
            }
            _ => Ok(push_semantic(session, SemanticOperator::Reverse, vec![term])),
        },
        SemanticOperator::Flatten => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { .. }) => {
                let mut out = Vec::new();
                flatten_collections_into(session, term, &mut out);
                Ok(push_list(session, out))
            }
            _ => Ok(push_semantic(session, SemanticOperator::Flatten, vec![term])),
        },
        SemanticOperator::Head => match session.arena.get(term) {
            // Ordered collections present as dialect `List`; Head returns that surface symbol.
            Some(athena_ir::TermNode::Collection { .. }) => Ok(push_symbol_name(session, "List")),
            Some(athena_ir::TermNode::Application { head, .. }) => match *head {
                athena_ir::ApplicationHead::Semantic(inner) => Ok(push_semantic(session, inner, Vec::new())),
                // Keep Extension identity; do not reify display_name into a Symbol atom.
                athena_ir::ApplicationHead::Extension(id) => Ok(push_extension(session, id, Vec::new())),
            },
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer().is_some() => {
                Ok(push_symbol_name(session, "Integer"))
            }
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(_))) => Ok(push_symbol_name(session, "Symbol")),
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::String(_))) => Ok(push_symbol_name(session, "String")),
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(_))) => Ok(push_symbol_name(session, "Symbol")),
            _ => Ok(push_semantic(session, SemanticOperator::Head, vec![term])),
        },
        _ => Err(diag("semantic_operator_not_implemented")),
    }
}

fn exact_floor(n: &Number) -> Option<Number> {
    match n {
        Number::Integer(_) => n.clone_inline(),
        Number::Rational(r) => Some(floor_rational(r)),
        Number::Real(Real::Machine(x)) => {
            let y = x.floor();
            if !y.is_finite() {
                return None;
            }
            if y.fract() == 0.0 && y.abs() <= i64::MAX as f64 {
                Some(Number::small_int(y as i64))
            }
            else {
                Some(Number::machine(y))
            }
        }
        _ => None,
    }
}

fn floor_rational(r: &Rational) -> Number {
    if r.is_integer() {
        return Number::Integer(r.numerator());
    }
    let numer = r.numerator();
    let denom = r.denominator();
    let (mut q, rem) = numer.div_rem_trunc(&denom).expect("rational denom non-zero");
    if !rem.is_zero() && q.is_negative() {
        q = q.sub(&Integer::one());
    }
    Number::Integer(q)
}

fn exact_ceiling(n: &Number) -> Option<Number> {
    match n {
        Number::Integer(_) => n.clone_inline(),
        Number::Rational(r) => Some(ceiling_rational(r)),
        Number::Real(Real::Machine(x)) => {
            let y = x.ceil();
            if !y.is_finite() {
                return None;
            }
            if y.fract() == 0.0 && y.abs() <= i64::MAX as f64 {
                Some(Number::small_int(y as i64))
            }
            else {
                Some(Number::machine(y))
            }
        }
        _ => None,
    }
}

fn ceiling_rational(r: &Rational) -> Number {
    if r.is_integer() {
        return Number::Integer(r.numerator());
    }
    let numer = r.numerator();
    let denom = r.denominator();
    let (mut q, rem) = numer.div_rem_trunc(&denom).expect("rational denom non-zero");
    if !rem.is_zero() && numer.is_non_negative() && q.is_non_negative() {
        q = q.add(&Integer::one());
    }
    Number::Integer(q)
}

fn exact_round(n: &Number) -> Option<Number> {
    match n {
        Number::Integer(_) => n.clone_inline(),
        Number::Rational(r) => Some(round_rational(r)),
        Number::Real(Real::Machine(x)) => {
            let y = round_half_to_even_f64(*x);
            if !y.is_finite() {
                return None;
            }
            if y.fract() == 0.0 && y.abs() <= i64::MAX as f64 {
                Some(Number::small_int(y as i64))
            }
            else {
                Some(Number::machine(y))
            }
        }
        _ => None,
    }
}

fn round_half_to_even_f64(x: f64) -> f64 {
    let fl = x.floor();
    let frac = x - fl;
    if frac < 0.5 {
        fl
    }
    else if frac > 0.5 {
        fl + 1.0
    }
    else if (fl as i64).rem_euclid(2) == 0 {
        fl
    }
    else {
        fl + 1.0
    }
}

fn exact_numerator(n: &Number) -> Option<Number> {
    match n {
        Number::Integer(_) => n.clone_inline(),
        Number::Rational(r) => Some(Number::Integer(r.numerator())),
        _ => None,
    }
}

fn exact_denominator(n: &Number) -> Option<Number> {
    match n {
        Number::Integer(_) => Some(Number::small_int(1)),
        Number::Rational(r) => Some(Number::Integer(r.denominator())),
        _ => None,
    }
}

fn exact_unit_step(n: &Number) -> Option<Number> {
    if n.is_zero() {
        return Some(Number::small_int(0));
    }
    if let Some(i) = n.as_exact_integer() {
        if i > 0 {
            return Some(Number::small_int(1));
        }
        if i < 0 {
            return Some(Number::small_int(0));
        }
    }
    if let Some(x) = n.as_machine_f64() {
        if x > 0.0 {
            return Some(Number::small_int(1));
        }
        if x < 0.0 {
            return Some(Number::small_int(0));
        }
        return Some(Number::small_int(0));
    }
    None
}

fn round_rational(r: &Rational) -> Number {
    if r.is_integer() {
        return Number::Integer(r.numerator());
    }
    let numer = r.numerator();
    let denom = r.denominator();
    let (q, rem) = numer.div_rem_trunc(&denom).expect("rational denom non-zero");
    if rem.is_zero() {
        return Number::Integer(q);
    }
    let twice_abs = rem.abs().mul(&Integer::from_i64(2));
    let denom_abs = denom.abs();
    let two = Integer::from_i64(2);
    match twice_abs.cmp(&denom_abs) {
        core::cmp::Ordering::Less => Number::Integer(q),
        core::cmp::Ordering::Greater => {
            if q.is_negative() {
                Number::Integer(q.sub(&Integer::one()))
            }
            else {
                Number::Integer(q.add(&Integer::one()))
            }
        }
        core::cmp::Ordering::Equal => {
            if q.rem_euclid(&two).expect("mod two").is_zero() {
                Number::Integer(q)
            }
            else if q.is_negative() {
                Number::Integer(q.sub(&Integer::one()))
            }
            else {
                Number::Integer(q.add(&Integer::one()))
            }
        }
    }
}

/// 递归展平有序集合元素（叶子非集合保留原样）。
fn flatten_collections_into(session: &Session, term: TermId, out: &mut Vec<TermId>) {
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => {
            for item in items {
                flatten_collections_into(session, *item, out);
            }
        }
        _ => out.push(term),
    }
}
