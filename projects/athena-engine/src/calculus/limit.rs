//! 极限求值 — 有限代入、单侧极点、多项式 ∞。

use athena_numeric::{Number, add as num_add, compare as num_compare, mul as num_mul};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode};

use crate::{
    eval::evaluate,
    term::{Term, number_from_term},
};

use super::{
    request::{LimitApproach, LimitDirection},
    result::CalculusResult,
    term_util::{contains_symbol, replace_symbol},
};

/// 在假设下尝试求极限。
pub fn limit_checked(
    expression: &Term,
    variable: &str,
    approach: &LimitApproach,
    direction: LimitDirection,
    _assumptions: &AssumptionSet,
) -> CalculusResult<Term> {
    match approach {
        LimitApproach::Finite(point) => limit_finite(expression, variable, point, direction),
        LimitApproach::PositiveInfinity => limit_infinity(expression, variable, true),
        LimitApproach::NegativeInfinity => limit_infinity(expression, variable, false),
    }
}

fn limit_finite(expression: &Term, variable: &str, point: &Term, direction: LimitDirection) -> CalculusResult<Term> {
    let substituted = replace_symbol(expression, variable, point);
    let value = evaluate(&substituted);

    if is_indeterminate_form(&value) {
        return CalculusResult::Unevaluated {
            expression: limit_form(expression, variable, &LimitApproach::Finite(point.clone()), direction),
            reason: Diagnostic::new(DiagnosticCode::LimitDoesNotExist),
        };
    }

    if is_singular_form(&value) {
        if direction != LimitDirection::TwoSided {
            if let Some(v) = try_onesided_simple_pole(expression, variable, point, direction) {
                return CalculusResult::Exact { value: v, conditions: Vec::new() };
            }
        }
        return CalculusResult::Unevaluated {
            expression: limit_form(expression, variable, &LimitApproach::Finite(point.clone()), direction),
            reason: Diagnostic::new(DiagnosticCode::LimitDoesNotExist),
        };
    }

    if !contains_symbol(&value, variable) && !is_open_limit_head(&value) {
        return CalculusResult::Exact { value, conditions: Vec::new() };
    }

    if direction != LimitDirection::TwoSided {
        if let Some(v) = try_onesided_simple_pole(expression, variable, point, direction) {
            return CalculusResult::Exact { value: v, conditions: Vec::new() };
        }
    }

    unevaluated_limit(expression, variable, &LimitApproach::Finite(point.clone()), direction)
}

/// 单侧简单极点 `c / (x - a)`（以及 `c / x`）。
fn try_onesided_simple_pole(expression: &Term, variable: &str, point: &Term, direction: LimitDirection) -> Option<Term> {
    let (num, den) = match expression {
        Term::Application { head, arguments: args } if head.is_symbol("Divide") && args.len() == 2 => {
            (args[0].clone(), args[1].clone())
        }
        Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 => {
            if number_from_term(&args[1]).is_some_and(|n| n.as_integer_exp() == Some(-1)) {
                (Term::int(1), args[0].clone())
            }
            else {
                return None;
            }
        }
        _ => return None,
    };

    let num_at = evaluate(&replace_symbol(&num, variable, point));
    let den_at = evaluate(&replace_symbol(&den, variable, point));
    let num_n = number_from_term(&num_at)?;
    let den_n = number_from_term(&den_at)?;
    if den_n.is_zero() && !num_n.is_zero() {
        // 在 point ± ε（ε = 1）探测 den，读取侧向符号。
        let eps = Term::int(1);
        let probe = match direction {
            LimitDirection::FromAbove => evaluate(&Term::apply("Plus", vec![point.clone(), eps])),
            LimitDirection::FromBelow => {
                evaluate(&Term::apply("Plus", vec![point.clone(), Term::apply("Times", vec![Term::int(-1), eps])]))
            }
            LimitDirection::TwoSided => return None,
        };
        let den_side = evaluate(&replace_symbol(&den, variable, &probe));
        let den_side_n = number_from_term(&den_side)?;
        let sign_den = num_compare(&den_side_n, &Number::small_int(0))?;
        let sign_num = num_compare(&num_n, &Number::small_int(0))?;
        use std::cmp::Ordering::*;
        let positive = match (sign_num, sign_den) {
            (Greater, Greater) | (Less, Less) => true,
            (Greater, Less) | (Less, Greater) => false,
            _ => return None,
        };
        return Some(if positive {
            Term::symbol("Infinity")
        }
        else {
            Term::apply("Times", vec![Term::int(-1), Term::symbol("Infinity")])
        });
    }
    None
}

fn limit_infinity(expression: &Term, variable: &str, positive: bool) -> CalculusResult<Term> {
    if let Some((degree, leading)) = polynomial_degree_leading(expression, variable) {
        if degree == 0 {
            return CalculusResult::Exact { value: Term::number(leading), conditions: Vec::new() };
        }
        if degree < 0 {
            return CalculusResult::Exact { value: Term::int(0), conditions: Vec::new() };
        }
        // 正次数：∞ → ∞·leading；负向趋近时用 (−∞)^degree。
        let mut sign_positive = num_compare(&leading, &Number::small_int(0)) == Some(std::cmp::Ordering::Greater);
        if leading.is_zero() {
            return unevaluated_limit(
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
        let value = if sign_positive {
            Term::symbol("Infinity")
        }
        else {
            Term::apply("Times", vec![Term::int(-1), Term::symbol("Infinity")])
        };
        return CalculusResult::Exact { value, conditions: Vec::new() };
    }
    unevaluated_limit(
        expression,
        variable,
        if positive { &LimitApproach::PositiveInfinity } else { &LimitApproach::NegativeInfinity },
        LimitDirection::TwoSided,
    )
}

/// 受限多项式语言的次数与首项系数。
fn polynomial_degree_leading(expr: &Term, var: &str) -> Option<(i64, Number)> {
    match expr {
        Term::Atom(_) if number_from_term(expr).is_some() => Some((0, number_from_term(expr)?.clone())),
        Term::Atom(_) if expr.is_symbol(var) => Some((1, Number::small_int(1))),
        Term::Atom(_) => None,
        Term::Application { head, arguments: args } => {
            let h = head.head_name()?;
            match h {
                "Plus" => {
                    let mut best: Option<(i64, Number)> = None;
                    for a in args {
                        let (d, c) = polynomial_degree_leading(a, var)?;
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
                        let (d, c) = polynomial_degree_leading(a, var)?;
                        deg += d;
                        coeff = num_mul(coeff, c).ok()?;
                    }
                    Some((deg, coeff))
                }
                "Power" if args.len() == 2 && args[0].is_symbol(var) => {
                    let n = number_from_term(&args[1])?.as_integer_exp()?;
                    Some((n, Number::small_int(1)))
                }
                "Subtract" if args.len() == 2 => polynomial_degree_leading(
                    &Term::apply("Plus", vec![args[0].clone(), Term::apply("Times", vec![Term::int(-1), args[1].clone()])]),
                    var,
                ),
                _ => None,
            }
        }
        Term::List(_) => None,
    }
}

fn is_open_limit_head(expr: &Term) -> bool {
    matches!(expr, Term::Application { head, .. } if head.is_symbol("Limit") || head.is_symbol("Indeterminate"))
}

fn limit_form(expression: &Term, variable: &str, approach: &LimitApproach, direction: LimitDirection) -> Term {
    let approach_term = match approach {
        LimitApproach::Finite(t) => t.clone(),
        LimitApproach::PositiveInfinity => Term::symbol("Infinity"),
        LimitApproach::NegativeInfinity => Term::apply("Times", vec![Term::int(-1), Term::symbol("Infinity")]),
    };
    let mut args = vec![expression.clone(), Term::List(vec![Term::symbol(variable), approach_term])];
    if direction != LimitDirection::TwoSided {
        args.push(Term::symbol(match direction {
            LimitDirection::FromBelow => "FromBelow",
            LimitDirection::FromAbove => "FromAbove",
            LimitDirection::TwoSided => unreachable!(),
        }));
    }
    Term::apply("Limit", args)
}

fn unevaluated_limit(
    expression: &Term,
    variable: &str,
    approach: &LimitApproach,
    direction: LimitDirection,
) -> CalculusResult<Term> {
    CalculusResult::Unevaluated {
        expression: limit_form(expression, variable, approach, direction),
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation),
    }
}

fn is_indeterminate_form(expr: &Term) -> bool {
    match expr {
        Term::Application { head, arguments: args } if head.is_symbol("Divide") && args.len() == 2 => {
            number_from_term(&args[0]).is_some_and(|n| n.is_zero()) && number_from_term(&args[1]).is_some_and(|n| n.is_zero())
        }
        Term::Application { head, .. } if head.is_symbol("Indeterminate") => true,
        _ => false,
    }
}

/// 非不定式奇异，如 `c/0` 或 `0^negative`。
fn is_singular_form(expr: &Term) -> bool {
    match expr {
        Term::Application { head, arguments: args } if head.is_symbol("Divide") && args.len() == 2 => {
            number_from_term(&args[1]).is_some_and(|n| n.is_zero()) && number_from_term(&args[0]).is_some_and(|n| !n.is_zero())
        }
        Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 => {
            number_from_term(&args[0]).is_some_and(|n| n.is_zero())
                && number_from_term(&args[1]).and_then(|e| e.as_integer_exp()).is_some_and(|e| e < 0)
        }
        _ => false,
    }
}
