//! 极限求值 — 有限代入、单侧极点、多项式 ∞（arena `TermId` · Living `25`）。

use athena_numeric::{Number, add as num_add, compare as num_compare, mul as num_mul};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, TermId};

use super::{
    ctx::CalculusCtx,
    request::{LimitApproach, LimitDirection},
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, replace_symbol},
};
use crate::execution::shape::Shape;

/// 在假设下尝试求极限。
pub fn limit_checked(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variable: &str,
    approach: &LimitApproach,
    direction: LimitDirection,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TermId> {
    match approach {
        LimitApproach::Finite(point) => limit_finite(cc, expression, variable, *point, direction),
        LimitApproach::PositiveInfinity => limit_infinity(cc, expression, variable, true),
        LimitApproach::NegativeInfinity => limit_infinity(cc, expression, variable, false),
    }
}

fn limit_finite(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variable: &str,
    point: TermId,
    direction: LimitDirection,
) -> CalculusResult<TermId> {
    if let Some(v) = try_known_finite_limit(cc, expression, variable, point) {
        return CalculusResult::Exact { value: v, conditions: Vec::new() };
    }

    let substituted = replace_symbol(cc, expression, variable, point);
    let value = cc.eval(substituted);

    let silent_zero_over_zero = cc.number_of(value).is_some_and(|n| n.is_zero())
        && split_quotient(cc, expression).is_some_and(|(num, den)| {
            let num_at = cc.eval(replace_symbol(cc, num, variable, point));
            let den_at = cc.eval(replace_symbol(cc, den, variable, point));
            cc.number_of(num_at).is_some_and(|n| n.is_zero()) && cc.number_of(den_at).is_some_and(|n| n.is_zero())
        });

    if is_indeterminate_form(cc, value) || silent_zero_over_zero {
        if let Some(v) = try_lhopital_once(cc, expression, variable, point, direction) {
            return CalculusResult::Exact { value: v, conditions: Vec::new() };
        }
        return CalculusResult::Unevaluated {
            expression: limit_form(cc, expression, variable, &LimitApproach::Finite(point), direction),
            reason: Diagnostic::new(DiagnosticCode::LimitDoesNotExist),
        };
    }

    if is_singular_form(cc, value) {
        if direction != LimitDirection::TwoSided {
            if let Some(v) = try_onesided_simple_pole(cc, expression, variable, point, direction) {
                return CalculusResult::Exact { value: v, conditions: Vec::new() };
            }
        }
        return CalculusResult::Unevaluated {
            expression: limit_form(cc, expression, variable, &LimitApproach::Finite(point), direction),
            reason: Diagnostic::new(DiagnosticCode::LimitDoesNotExist),
        };
    }

    if !contains_symbol(cc, value, variable) && !is_open_limit_head(cc, value) {
        return CalculusResult::Exact { value, conditions: Vec::new() };
    }

    if direction != LimitDirection::TwoSided {
        if let Some(v) = try_onesided_simple_pole(cc, expression, variable, point, direction) {
            return CalculusResult::Exact { value: v, conditions: Vec::new() };
        }
    }

    unevaluated_limit(cc, expression, variable, &LimitApproach::Finite(point), direction)
}

fn try_known_finite_limit(cc: &CalculusCtx<'_>, expression: TermId, variable: &str, point: TermId) -> Option<TermId> {
    if !cc.number_of(point).is_some_and(|n| n.is_zero()) {
        return None;
    }
    if is_sinc_form(cc, expression, variable) {
        return Some(cc.in_(1));
    }
    None
}

fn is_sinc_form(cc: &CalculusCtx<'_>, expression: TermId, variable: &str) -> bool {
    let Some((h, args)) = cc.application(expression)
    else {
        return false;
    };
    match h.as_str() {
        "Divide" if args.len() == 2 => is_sin_of_var(cc, args[0], variable) && is_symbol_named(cc, args[1], variable),
        "Times" if args.len() == 2 => {
            (is_sin_of_var(cc, args[0], variable) && is_reciprocal_var(cc, args[1], variable))
                || (is_sin_of_var(cc, args[1], variable) && is_reciprocal_var(cc, args[0], variable))
        }
        _ => false,
    }
}

fn is_sin_of_var(cc: &CalculusCtx<'_>, expr: TermId, variable: &str) -> bool {
    matches!(cc.application(expr), Some((h, args)) if h == "Sin" && args.len() == 1 && is_symbol_named(cc, args[0], variable))
}

fn is_reciprocal_var(cc: &CalculusCtx<'_>, expr: TermId, variable: &str) -> bool {
    matches!(
        cc.application(expr),
        Some((h, args))
            if h == "Power"
                && args.len() == 2
                && is_symbol_named(cc, args[0], variable)
                && cc.int_exp(args[1]) == Some(-1)
    )
}

fn try_lhopital_once(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variable: &str,
    point: TermId,
    _direction: LimitDirection,
) -> Option<TermId> {
    let (num, den) = split_quotient(cc, expression)?;
    let num_at = cc.eval(replace_symbol(cc, num, variable, point));
    let den_at = cc.eval(replace_symbol(cc, den, variable, point));
    let num_zero = cc.number_of(num_at).is_some_and(|n| n.is_zero());
    let den_zero = cc.number_of(den_at).is_some_and(|n| n.is_zero());
    if !(num_zero && den_zero) {
        return None;
    }
    let num_d = super::derivative::differentiate(cc, num, variable);
    let den_d = super::derivative::differentiate(cc, den, variable);
    let inv = cc.apply("Power", vec![den_d, cc.in_(-1)]);
    let ratio = cc.apply("Times", vec![num_d, inv]);
    let value = cc.eval(replace_symbol(cc, ratio, variable, point));
    if is_indeterminate_form(cc, value) || is_singular_form(cc, value) || contains_symbol(cc, value, variable) {
        return None;
    }
    Some(value)
}

fn split_quotient(cc: &CalculusCtx<'_>, expression: TermId) -> Option<(TermId, TermId)> {
    let (h, args) = cc.application(expression)?;
    match h.as_str() {
        "Divide" if args.len() == 2 => Some((args[0], args[1])),
        "Times" if args.len() == 2 => {
            if is_reciprocal_power(cc, args[1]) {
                Some((args[0], reciprocal_base(cc, args[1])?))
            }
            else if is_reciprocal_power(cc, args[0]) {
                Some((args[1], reciprocal_base(cc, args[0])?))
            }
            else {
                None
            }
        }
        _ => None,
    }
}

fn is_reciprocal_power(cc: &CalculusCtx<'_>, expr: TermId) -> bool {
    matches!(cc.application(expr), Some((h, args)) if h == "Power" && args.len() == 2 && cc.int_exp(args[1]) == Some(-1))
}

fn reciprocal_base(cc: &CalculusCtx<'_>, expr: TermId) -> Option<TermId> {
    match cc.application(expr) {
        Some((h, args)) if h == "Power" && args.len() == 2 && cc.int_exp(args[1]) == Some(-1) => Some(args[0]),
        _ => None,
    }
}

fn try_onesided_simple_pole(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variable: &str,
    point: TermId,
    direction: LimitDirection,
) -> Option<TermId> {
    let (num, den) = match cc.application(expression) {
        Some((h, args)) if h == "Divide" && args.len() == 2 => (args[0], args[1]),
        Some((h, args)) if h == "Power" && args.len() == 2 => {
            if cc.int_exp(args[1]) == Some(-1) {
                (cc.in_(1), args[0])
            }
            else {
                return None;
            }
        }
        _ => return None,
    };

    let num_at = cc.eval(replace_symbol(cc, num, variable, point));
    let den_at = cc.eval(replace_symbol(cc, den, variable, point));
    let num_n = cc.number_of(num_at).map(|n| cc.copy(n))?;
    let den_n = cc.number_of(den_at).map(|n| cc.copy(n))?;
    if den_n.is_zero() && !num_n.is_zero() {
        let eps = cc.in_(1);
        let probe = match direction {
            LimitDirection::FromAbove => cc.eval(cc.apply("Plus", vec![point, eps])),
            LimitDirection::FromBelow => {
                let neg = cc.apply("Times", vec![cc.in_(-1), eps]);
                cc.eval(cc.apply("Plus", vec![point, neg]))
            }
            LimitDirection::TwoSided => return None,
        };
        let den_side = cc.eval(replace_symbol(cc, den, variable, probe));
        let den_side_n = cc.number_of(den_side).map(|n| cc.copy(n))?;
        let sign_den = num_compare(&den_side_n, &Number::small_int(0))?;
        let sign_num = num_compare(&num_n, &Number::small_int(0))?;
        use std::cmp::Ordering::*;
        let positive = match (sign_num, sign_den) {
            (Greater, Greater) | (Less, Less) => true,
            (Greater, Less) | (Less, Greater) => false,
            _ => return None,
        };
        return Some(if positive { cc.symbol("Infinity") } else { cc.apply("Times", vec![cc.in_(-1), cc.symbol("Infinity")]) });
    }
    None
}

fn limit_infinity(cc: &mut CalculusCtx<'_>, expression: TermId, variable: &str, positive: bool) -> CalculusResult<TermId> {
    if let Some((degree, leading)) = polynomial_degree_leading(cc, expression, variable) {
        if degree == 0 {
            return CalculusResult::Exact { value: cc.num(leading), conditions: Vec::new() };
        }
        if degree < 0 {
            return CalculusResult::Exact { value: cc.in_(0), conditions: Vec::new() };
        }
        let mut sign_positive = num_compare(&leading, &Number::small_int(0)) == Some(std::cmp::Ordering::Greater);
        if leading.is_zero() {
            return unevaluated_limit(
                cc,
                expression,
                variable,
                if positive { &LimitApproach::PositiveInfinity } else { &LimitApproach::NegativeInfinity },
                LimitDirection::TwoSided,
            );
        }
        if num_compare(&leading, &Number::small_int(0)) == Some(std::cmp::Ordering::Less) {
            sign_positive = false;
        }
        if !positive && degree % 2 == 1 {
            sign_positive = !sign_positive;
        }
        let value = if sign_positive { cc.symbol("Infinity") } else { cc.apply("Times", vec![cc.in_(-1), cc.symbol("Infinity")]) };
        return CalculusResult::Exact { value, conditions: Vec::new() };
    }
    unevaluated_limit(
        cc,
        expression,
        variable,
        if positive { &LimitApproach::PositiveInfinity } else { &LimitApproach::NegativeInfinity },
        LimitDirection::TwoSided,
    )
}

fn polynomial_degree_leading(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> Option<(i64, Number)> {
    match cc.shape(expr)? {
        Shape::Number => Some((0, cc.number_of(expr).map(|n| cc.copy(n))?)),
        Shape::Symbol(s) if cc.symbol_is(s, var) => Some((1, Number::small_int(1))),
        Shape::Symbol(_) | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::List(_) => None,
        Shape::Application(_, _) => {
            let (h, args) = cc.application(expr)?;
            match h.as_str() {
                "Plus" => {
                    let mut best: Option<(i64, Number)> = None;
                    for a in args {
                        let (d, c) = polynomial_degree_leading(cc, a, var)?;
                        best = match best {
                            None => Some((d, c)),
                            Some((bd, _bc)) if d > bd => Some((d, c)),
                            Some((bd, bc)) if d == bd => Some((bd, num_add(bc, c).ok()?)),
                            Some(b) => Some(b),
                        };
                    }
                    best
                }
                "Times" => {
                    let mut deg = 0i64;
                    let mut coeff = Number::small_int(1);
                    for a in args {
                        let (d, c) = polynomial_degree_leading(cc, a, var)?;
                        deg += d;
                        coeff = num_mul(coeff, c).ok()?;
                    }
                    Some((deg, coeff))
                }
                "Power" if args.len() == 2 && is_symbol_named(cc, args[0], var) => {
                    let n = cc.int_exp(args[1])?;
                    Some((n, Number::small_int(1)))
                }
                "Subtract" if args.len() == 2 => {
                    let neg = cc.apply("Times", vec![cc.in_(-1), args[1]]);
                    let rewritten = cc.apply("Plus", vec![args[0], neg]);
                    polynomial_degree_leading(cc, rewritten, var)
                }
                _ => None,
            }
        }
    }
}

fn is_open_limit_head(cc: &CalculusCtx<'_>, expr: TermId) -> bool {
    matches!(cc.head_name(expr).as_deref(), Some("Limit") | Some("Indeterminate"))
}

fn limit_form(cc: &mut CalculusCtx<'_>, expression: TermId, variable: &str, approach: &LimitApproach, direction: LimitDirection) -> TermId {
    let approach_term = match approach {
        LimitApproach::Finite(t) => *t,
        LimitApproach::PositiveInfinity => cc.symbol("Infinity"),
        LimitApproach::NegativeInfinity => cc.apply("Times", vec![cc.in_(-1), cc.symbol("Infinity")]),
    };
    let spec = cc.list(vec![cc.symbol(variable), approach_term]);
    let mut args = vec![expression, spec];
    if direction != LimitDirection::TwoSided {
        args.push(cc.symbol(match direction {
            LimitDirection::FromBelow => "FromBelow",
            LimitDirection::FromAbove => "FromAbove",
            LimitDirection::TwoSided => unreachable!(),
        }));
    }
    cc.apply("Limit", args)
}

fn unevaluated_limit(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variable: &str,
    approach: &LimitApproach,
    direction: LimitDirection,
) -> CalculusResult<TermId> {
    CalculusResult::Unevaluated {
        expression: limit_form(cc, expression, variable, approach, direction),
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation),
    }
}

fn is_indeterminate_form(cc: &CalculusCtx<'_>, expr: TermId) -> bool {
    let Some((h, args)) = cc.application(expr)
    else {
        return false;
    };
    match h.as_str() {
        "Divide" if args.len() == 2 => cc.number_of(args[0]).is_some_and(|n| n.is_zero()) && cc.number_of(args[1]).is_some_and(|n| n.is_zero()),
        "Times" => {
            let has_zero = args.iter().any(|a| cc.number_of(*a).is_some_and(|n| n.is_zero()));
            let has_singular_pow = args.iter().any(|a| {
                matches!(
                    cc.application(*a),
                    Some((ph, p))
                        if ph == "Power"
                            && p.len() == 2
                            && cc.number_of(p[0]).is_some_and(|n| n.is_zero())
                            && cc.int_exp(p[1]).is_some_and(|e| e < 0)
                )
            });
            has_zero && has_singular_pow
        }
        "Indeterminate" => true,
        _ => false,
    }
}

fn is_singular_form(cc: &CalculusCtx<'_>, expr: TermId) -> bool {
    let Some((h, args)) = cc.application(expr)
    else {
        return false;
    };
    match h.as_str() {
        "Divide" if args.len() == 2 => {
            cc.number_of(args[1]).is_some_and(|n| n.is_zero()) && cc.number_of(args[0]).is_some_and(|n| !n.is_zero())
        }
        "Power" if args.len() == 2 => cc.number_of(args[0]).is_some_and(|n| n.is_zero()) && cc.int_exp(args[1]).is_some_and(|e| e < 0),
        _ => false,
    }
}

fn is_symbol_named(cc: &CalculusCtx<'_>, term: TermId, name: &str) -> bool {
    matches!(cc.shape(term), Some(Shape::Symbol(s)) if cc.symbol_is(s, name))
}
