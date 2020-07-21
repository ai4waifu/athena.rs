//! 会话 arena 上的不定 / 定积分（初等子集 · `DomainExecutionContext` · ）。

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_types::{Diagnostic, DiagnosticCode, Result, SymbolId, TermId};

use super::{
    derivative::differentiate,
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, is_symbol_id, replace_symbol},
};
use crate::{domains::context::DomainExecutionContext, execution::shape::Shape};

/// 在 arena 上做符号积分（多项式 / 初等子集）。
pub fn integrate(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Result<TermId> {
    integrate_symbol(dc, expr, var)
}

fn integrate_symbol(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Result<TermId> {
    let Some(shape) = dc.shape(expr)
    else {
        return Ok(expr);
    };
    Ok(match shape {
        Shape::Number => {
            let n = dc.number_of(expr).expect("number");
            dc.apply_semantic(SemanticOperator::Multiply, vec![dc.num(n), dc.symbol_id(var)])
        }
        Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => residual_integrate(dc, expr, var),
        Shape::Symbol(s) => {
            if dc.symbol_id_is(s, var) {
                let x2 = dc.apply_semantic(SemanticOperator::Power, vec![dc.symbol_id(var), dc.in_(2)]);
                dc.fold_term(dc.apply_semantic(SemanticOperator::Divide, vec![x2, dc.in_(2)]))?
            }
            else {
                dc.apply_semantic(SemanticOperator::Multiply, vec![expr, dc.symbol_id(var)])
            }
        }
        Shape::Collection(items) => {
            let iss: Result<Vec<_>> = items.iter().map(|i| integrate_symbol(dc, *i, var)).collect();
            dc.ordered(iss?)
        }
        Shape::Application(head, args) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
                let iss: Result<Vec<_>> = args.iter().map(|a| integrate_symbol(dc, *a, var)).collect();
                dc.fold_term(dc.apply_semantic(SemanticOperator::Add, iss?))?
            }
            ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
                if let Some((c, r)) = split_numeric_factor(dc, args[0], args[1]) {
                    let ir = integrate_symbol(dc, r, var)?;
                    dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![c, ir]))?
                }
                else if let Some(parts) = try_integrate_by_parts(dc, args[0], args[1], var)? {
                    parts
                }
                else {
                    residual_integrate(dc, expr, var)
                }
            }
            ApplicationHead::Semantic(SemanticOperator::Divide) if args.len() == 2 => {
                let inv = dc.apply_semantic(SemanticOperator::Power, vec![args[1], dc.in_(-1)]);
                let rewritten = dc.apply_semantic(SemanticOperator::Multiply, vec![args[0], inv]);
                integrate_symbol(dc, rewritten, var)?
            }
            ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 && is_symbol_id(dc, args[0], var) => {
                if let Some(n) = dc.int_exp(args[1]) {
                    if n == -1 {
                        return Ok(dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Log), vec![args[0]]));
                    }
                    let p = dc.apply_semantic(SemanticOperator::Power, vec![args[0], dc.in_(n + 1)]);
                    return Ok(dc.fold_term(dc.apply_semantic(SemanticOperator::Divide, vec![p, dc.in_(n + 1)]))?);
                }
                residual_integrate(dc, expr, var)
            }
            ApplicationHead::Semantic(op) => {
                if let Some(uf) = op.as_unary() {
                    if args.len() == 1 && is_symbol_id(dc, args[0], var) {
                        match uf {
                            UnaryFunction::Sin => {
                                let c = dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Cos), args.clone());
                                return Ok(dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), c]))?);
                            }
                            UnaryFunction::Cos => {
                                return Ok(dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Sin), args.clone()));
                            }
                            UnaryFunction::Exp => {
                                return Ok(dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Exp), args.clone()));
                            }
                            _ => {}
                        }
                    }
                }
                residual_integrate(dc, expr, var)
            }
            ApplicationHead::Extension(_) => residual_integrate(dc, expr, var),
        },
    })
}

fn split_numeric_factor(dc: &DomainExecutionContext<'_>, a: TermId, b: TermId) -> Option<(TermId, TermId)> {
    if dc.number_of(a).is_some() {
        Some((a, b))
    }
    else if dc.number_of(b).is_some() {
        Some((b, a))
    }
    else {
        None
    }
}

/// `∫ u dv` with `u` a power of `var` and `dv` = `Sin`/`Cos` of `var`.
fn try_integrate_by_parts(dc: &mut DomainExecutionContext<'_>, a: TermId, b: TermId, var: SymbolId) -> Result<Option<TermId>> {
    let (u, dv) = if is_poly_power_of_var(dc, a, var) && is_sin_or_cos_of_var(dc, b, var) {
        (a, b)
    }
    else if is_poly_power_of_var(dc, b, var) && is_sin_or_cos_of_var(dc, a, var) {
        (b, a)
    }
    else {
        return Ok(None);
    };
    let du = differentiate(dc, u, var)?;
    let v = integrate_symbol(dc, dv, var)?;
    if is_integrate_residual(dc, v) {
        return Ok(None);
    }
    let uv = dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![u, v]))?;
    let v_du = dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![v, du]))?;
    let int_v_du = integrate_symbol(dc, v_du, var)?;
    if is_integrate_residual(dc, int_v_du) {
        return Ok(None);
    }
    let minus = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), int_v_du]);
    Ok(Some(dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![uv, minus]))?))
}

fn is_poly_power_of_var(dc: &DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> bool {
    if is_symbol_id(dc, expr, var) {
        return true;
    }
    matches!(
        dc.application_head(expr),
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args))
            if args.len() == 2 && is_symbol_id(dc, args[0], var) && dc.int_exp(args[1]).is_some_and(|n| n >= 1)
    )
}

fn is_sin_or_cos_of_var(dc: &DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> bool {
    matches!(
        dc.application_head(expr),
        Some((ApplicationHead::Semantic(op), args))
            if args.len() == 1
                && is_symbol_id(dc, args[0], var)
                && matches!(op.as_unary(), Some(UnaryFunction::Sin | UnaryFunction::Cos))
    )
}

fn residual_integrate(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> TermId {
    dc.apply_semantic(SemanticOperator::Integrate, vec![expr, dc.symbol_id(var)])
}

fn is_integrate_residual(dc: &DomainExecutionContext<'_>, value: TermId) -> bool {
    matches!(dc.application_head(value), Some((ApplicationHead::Semantic(SemanticOperator::Integrate), _)))
}

/// 积分并包装为 [`CalculusResult`]（初等 vs 未求值）。
pub fn integrate_checked(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> Result<CalculusResult<TermId>> {
    let value = integrate(dc, expr, var)?;
    Ok(if is_integrate_residual(dc, value) {
        CalculusResult::Unevaluated { expression: value, reason: Diagnostic::new(DiagnosticCode::IntegralNotElementary) }
    }
    else {
        CalculusResult::Exact { value, conditions: Vec::new() }
    })
}

/// 经原函数求值 `F(upper) - F(lower)` 的定积分。
pub fn definite_integrate_checked(
    dc: &mut DomainExecutionContext<'_>,
    expr: TermId,
    var: SymbolId,
    lower: TermId,
    upper: TermId,
) -> Result<CalculusResult<TermId>> {
    Ok(match integrate_checked(dc, expr, var)? {
        CalculusResult::Exact { value: anti, conditions } => {
            let at_upper = dc.fold_term(replace_symbol(dc, anti, var, upper))?;
            let at_lower = dc.fold_term(replace_symbol(dc, anti, var, lower))?;
            let neg = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), at_lower]);
            let value = dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![at_upper, neg]))?;
            if contains_symbol(dc, value, var) {
                CalculusResult::Unevaluated {
                    expression: residual_definite(dc, expr, var, lower, upper),
                    reason: Diagnostic::new(DiagnosticCode::IntegralNotElementary),
                }
            }
            else {
                CalculusResult::Exact { value, conditions }
            }
        }
        CalculusResult::Conditional { value: anti, conditions } => {
            let at_upper = dc.fold_term(replace_symbol(dc, anti, var, upper))?;
            let at_lower = dc.fold_term(replace_symbol(dc, anti, var, lower))?;
            let neg = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), at_lower]);
            let value = dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![at_upper, neg]))?;
            CalculusResult::Conditional { value, conditions }
        }
        CalculusResult::Unevaluated { .. } => CalculusResult::Unevaluated {
            expression: residual_definite(dc, expr, var, lower, upper),
            reason: Diagnostic::new(DiagnosticCode::IntegralNotElementary),
        },
    })
}

fn residual_definite(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId, lower: TermId, upper: TermId) -> TermId {
    let iter = dc.ordered(vec![dc.symbol_id(var), lower, upper]);
    dc.apply_semantic(SemanticOperator::Integrate, vec![expr, iter])
}
