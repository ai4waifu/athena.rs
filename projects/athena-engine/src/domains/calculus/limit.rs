//! 极限求值 — 有限代入、单侧极点、多项式 ∞（arena `TermId` · Living `25`）。

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_numeric::{Number, add as num_add, compare as num_compare, mul as num_mul};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, SymbolId, TermId};

use super::{
    request::{LimitApproach, LimitDirection},
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, is_symbol_id, replace_symbol},
};
use crate::{domains::context::DomainExecutionContext, execution::shape::Shape};

/// 在假设下尝试求极限。
pub fn limit_checked(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
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
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
    point: TermId,
    direction: LimitDirection,
) -> CalculusResult<TermId> {
    if let Some(v) = try_known_finite_limit(cc, expression, variable, point) {
        return CalculusResult::Exact { value: v, conditions: Vec::new() };
    }

    let substituted = replace_symbol(cc, expression, variable, point);
    let value = cc.fold_term(substituted);

    let silent_zero_over_zero = cc.number_of(value).is_some_and(|n| n.is_zero())
        && split_quotient(cc, expression).is_some_and(|(num, den)| {
            let num_at = cc.fold_term(replace_symbol(cc, num, variable, point));
            let den_at = cc.fold_term(replace_symbol(cc, den, variable, point));
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

fn try_known_finite_limit(cc: &DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, point: TermId) -> Option<TermId> {
    if !cc.number_of(point).is_some_and(|n| n.is_zero()) {
        return None;
    }
    if is_sinc_form(cc, expression, variable) {
        return Some(cc.in_(1));
    }
    None
}

fn is_sinc_form(cc: &DomainExecutionContext<'_>, expression: TermId, variable: SymbolId) -> bool {
    let Some((head, args)) = cc.application_head(expression)
    else {
        return false;
    };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => {
            is_sin_of_var(cc, args[0], variable) && is_symbol_id(cc, args[1], variable)
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
            (is_sin_of_var(cc, args[0], variable) && is_reciprocal_var(cc, args[1], variable))
                || (is_sin_of_var(cc, args[1], variable) && is_reciprocal_var(cc, args[0], variable))
        }
        _ => false,
    }
}

fn is_sin_of_var(cc: &DomainExecutionContext<'_>, expr: TermId, variable: SymbolId) -> bool {
    matches!(
        cc.application_head(expr),
        Some((ApplicationHead::Semantic(op), args))
            if op.as_unary() == Some(UnaryFunction::Sin) && args.len() == 1 && is_symbol_id(cc, args[0], variable)
    )
}

fn is_reciprocal_var(cc: &DomainExecutionContext<'_>, expr: TermId, variable: SymbolId) -> bool {
    matches!(
        cc.application_head(expr),
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args))
            if args.len() == 2 && is_symbol_id(cc, args[0], variable) && cc.int_exp(args[1]) == Some(-1)
    )
}

fn try_lhopital_once(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
    point: TermId,
    _direction: LimitDirection,
) -> Option<TermId> {
    let (num, den) = split_quotient(cc, expression)?;
    let num_at = cc.fold_term(replace_symbol(cc, num, variable, point));
    let den_at = cc.fold_term(replace_symbol(cc, den, variable, point));
    let num_zero = cc.number_of(num_at).is_some_and(|n| n.is_zero());
    let den_zero = cc.number_of(den_at).is_some_and(|n| n.is_zero());
    if !(num_zero && den_zero) {
        return None;
    }
    let num_d = super::derivative::differentiate(cc, num, variable);
    let den_d = super::derivative::differentiate(cc, den, variable);
    let inv = cc.apply_semantic(SemanticOperator::Power, vec![den_d, cc.in_(-1)]);
    let ratio = cc.apply_semantic(SemanticOperator::Multiply, vec![num_d, inv]);
    let value = cc.fold_term(replace_symbol(cc, ratio, variable, point));
    if is_indeterminate_form(cc, value) || is_singular_form(cc, value) || contains_symbol(cc, value, variable) {
        return None;
    }
    Some(value)
}

fn split_quotient(cc: &DomainExecutionContext<'_>, expression: TermId) -> Option<(TermId, TermId)> {
    let (head, args) = cc.application_head(expression)?;
    match head {
        ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => Some((args[0], args[1])),
        ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
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

fn is_reciprocal_power(cc: &DomainExecutionContext<'_>, expr: TermId) -> bool {
    matches!(
        cc.application_head(expr),
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args)) if args.len() == 2 && cc.int_exp(args[1]) == Some(-1)
    )
}

fn reciprocal_base(cc: &DomainExecutionContext<'_>, expr: TermId) -> Option<TermId> {
    match cc.application_head(expr) {
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args)) if args.len() == 2 && cc.int_exp(args[1]) == Some(-1) => Some(args[0]),
        _ => None,
    }
}

fn try_onesided_simple_pole(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
    point: TermId,
    direction: LimitDirection,
) -> Option<TermId> {
    let (num, den) = match cc.application_head(expression) {
        Some((ApplicationHead::Semantic(SemanticOperator::Divide), args)) if args.len() == 2 => (args[0], args[1]),
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args)) if args.len() == 2 => {
            if cc.int_exp(args[1]) == Some(-1) {
                (cc.in_(1), args[0])
            }
            else {
                return None;
            }
        }
        _ => return None,
    };

    let num_at = cc.fold_term(replace_symbol(cc, num, variable, point));
    let den_at = cc.fold_term(replace_symbol(cc, den, variable, point));
    let num_n = cc.number_of(num_at).map(|n| cc.copy(n))?;
    let den_n = cc.number_of(den_at).map(|n| cc.copy(n))?;
    if den_n.is_zero() && !num_n.is_zero() {
        let eps = cc.in_(1);
        let probe = match direction {
            LimitDirection::FromAbove => cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![point, eps])),
            LimitDirection::FromBelow => {
                let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), eps]);
                cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![point, neg]))
            }
            LimitDirection::TwoSided => return None,
        };
        let den_side = cc.fold_term(replace_symbol(cc, den, variable, probe));
        let den_side_n = cc.number_of(den_side).map(|n| cc.copy(n))?;
        let sign_den = num_compare(&den_side_n, &Number::small_int(0))?;
        let sign_num = num_compare(&num_n, &Number::small_int(0))?;
        use std::cmp::Ordering::*;
        let positive = match (sign_num, sign_den) {
            (Greater, Greater) | (Less, Less) => true,
            (Greater, Less) | (Less, Greater) => false,
            _ => return None,
        };
        return Some(if positive {
            cc.symbol("Infinity")
        }
        else {
            cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), cc.symbol("Infinity")])
        });
    }
    None
}

fn limit_infinity(cc: &mut DomainExecutionContext<'_>, expression: TermId, variable: SymbolId, positive: bool) -> CalculusResult<TermId> {
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
        let value = if sign_positive {
            cc.symbol("Infinity")
        }
        else {
            cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), cc.symbol("Infinity")])
        };
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

fn polynomial_degree_leading(cc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Option<(i64, Number)> {
    match cc.shape(expr)? {
        Shape::Number => Some((0, cc.number_of(expr).map(|n| cc.copy(n))?)),
        Shape::Symbol(s) if cc.symbol_id_is(s, var) => Some((1, Number::small_int(1))),
        Shape::Symbol(_) | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) | Shape::Collection(_) => None,
        Shape::Application(head, args) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
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
            ApplicationHead::Semantic(SemanticOperator::Multiply) => {
                let mut deg = 0i64;
                let mut coeff = Number::small_int(1);
                for a in args {
                    let (d, c) = polynomial_degree_leading(cc, a, var)?;
                    deg += d;
                    coeff = num_mul(coeff, c).ok()?;
                }
                Some((deg, coeff))
            }
            ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 && is_symbol_id(cc, args[0], var) => {
                let n = cc.int_exp(args[1])?;
                Some((n, Number::small_int(1)))
            }
            ApplicationHead::Semantic(SemanticOperator::Subtract) if args.len() == 2 => {
                let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), args[1]]);
                let rewritten = cc.apply_semantic(SemanticOperator::Add, vec![args[0], neg]);
                polynomial_degree_leading(cc, rewritten, var)
            }
            _ => None,
        },
    }
}

fn is_open_limit_head(cc: &DomainExecutionContext<'_>, expr: TermId) -> bool {
    match cc.application_head(expr) {
        Some((ApplicationHead::Semantic(SemanticOperator::Limit), _)) => true,
        Some((head, _)) if cc.is_indeterminate_extension(head) => true,
        _ => false,
    }
}

fn limit_form(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
    approach: &LimitApproach,
    direction: LimitDirection,
) -> TermId {
    let approach_term = match approach {
        LimitApproach::Finite(t) => *t,
        LimitApproach::PositiveInfinity => cc.symbol("Infinity"),
        LimitApproach::NegativeInfinity => cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), cc.symbol("Infinity")]),
    };
    let spec = cc.ordered(vec![cc.symbol_id(variable), approach_term]);
    let mut args = vec![expression, spec];
    if direction != LimitDirection::TwoSided {
        args.push(cc.symbol(match direction {
            LimitDirection::FromBelow => "FromBelow",
            LimitDirection::FromAbove => "FromAbove",
            LimitDirection::TwoSided => unreachable!(),
        }));
    }
    cc.apply_semantic(SemanticOperator::Limit, args)
}

fn unevaluated_limit(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    variable: SymbolId,
    approach: &LimitApproach,
    direction: LimitDirection,
) -> CalculusResult<TermId> {
    CalculusResult::Unevaluated {
        expression: limit_form(cc, expression, variable, approach, direction),
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation),
    }
}

fn is_indeterminate_form(cc: &DomainExecutionContext<'_>, expr: TermId) -> bool {
    let Some((head, args)) = cc.application_head(expr)
    else {
        return false;
    };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => {
            cc.number_of(args[0]).is_some_and(|n| n.is_zero()) && cc.number_of(args[1]).is_some_and(|n| n.is_zero())
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) => {
            let has_zero = args.iter().any(|a| cc.number_of(*a).is_some_and(|n| n.is_zero()));
            let has_singular_pow = args.iter().any(|a| {
                matches!(
                    cc.application_head(*a),
                    Some((ApplicationHead::Semantic(SemanticOperator::Power), p))
                        if p.len() == 2
                            && cc.number_of(p[0]).is_some_and(|n| n.is_zero())
                            && cc.int_exp(p[1]).is_some_and(|e| e < 0)
                )
            });
            has_zero && has_singular_pow
        }
        head if cc.is_indeterminate_extension(head) => true,
        _ => false,
    }
}

fn is_singular_form(cc: &DomainExecutionContext<'_>, expr: TermId) -> bool {
    let Some((head, args)) = cc.application_head(expr)
    else {
        return false;
    };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => {
            cc.number_of(args[1]).is_some_and(|n| n.is_zero()) && cc.number_of(args[0]).is_some_and(|n| !n.is_zero())
        }
        ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 => {
            cc.number_of(args[0]).is_some_and(|n| n.is_zero()) && cc.int_exp(args[1]).is_some_and(|e| e < 0)
        }
        _ => false,
    }
}
