//! 会话 arena 上的不定 / 定积分（初等子集 · `DomainExecutionContext` · Living `27`）。

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_types::{Diagnostic, DiagnosticCode, SymbolId, TermId};

use super::{
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, is_symbol_id, replace_symbol},
};
use crate::domains::context::DomainExecutionContext;
use crate::execution::shape::Shape;

/// 在 arena 上做符号积分（多项式 / 初等子集）。
pub fn integrate(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> TermId {
    integrate_symbol(dc, expr, var)
}

fn integrate_symbol(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> TermId {
    let Some(shape) = dc.shape(expr)
    else {
        return expr;
    };
    match shape {
        Shape::Number => {
            let n = dc.number_of(expr).map(|n| dc.copy(n)).expect("number");
            dc.apply_semantic(SemanticOperator::Multiply, vec![dc.num(n), dc.symbol_id(var)])
        }
        Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => residual_integrate(dc, expr, var),
        Shape::Symbol(s) => {
            if dc.symbol_id_is(s, var) {
                let x2 = dc.apply_semantic(SemanticOperator::Power, vec![dc.symbol_id(var), dc.in_(2)]);
                dc.fold_term(dc.apply_semantic(SemanticOperator::Divide, vec![x2, dc.in_(2)]))
            }
            else {
                dc.apply_semantic(SemanticOperator::Multiply, vec![expr, dc.symbol_id(var)])
            }
        }
        Shape::Collection(items) => {
            let iss = items.iter().map(|i| integrate_symbol(dc, *i, var)).collect();
            dc.ordered(iss)
        }
        Shape::Application(head, args) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
                let iss = args.iter().map(|a| integrate_symbol(dc, *a, var)).collect();
                dc.fold_term(dc.apply_semantic(SemanticOperator::Add, iss))
            }
            ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
                let (coeff, rest) = if dc.number_of(args[0]).is_some() {
                    (args[0], args[1])
                }
                else if dc.number_of(args[1]).is_some() {
                    (args[1], args[0])
                }
                else {
                    return residual_integrate(dc, expr, var);
                };
                let ir = integrate_symbol(dc, rest, var);
                dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![coeff, ir]))
            }
            ApplicationHead::Semantic(SemanticOperator::Power)
                if args.len() == 2 && is_symbol_id(dc, args[0], var) =>
            {
                if let Some(n) = dc.int_exp(args[1]) {
                    if n != -1 {
                        let p = dc.apply_semantic(SemanticOperator::Power, vec![args[0], dc.in_(n + 1)]);
                        return dc.fold_term(dc.apply_semantic(SemanticOperator::Divide, vec![p, dc.in_(n + 1)]));
                    }
                }
                residual_integrate(dc, expr, var)
            }
            ApplicationHead::Semantic(op) => {
                if let Some(uf) = op.as_unary() {
                    if args.len() == 1 && is_symbol_id(dc, args[0], var) {
                        match uf {
                            UnaryFunction::Sin => {
                                let c = dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Cos), args.clone());
                                return dc.fold_term(dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), c]));
                            }
                            UnaryFunction::Cos => {
                                return dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Sin), args.clone());
                            }
                            UnaryFunction::Exp => {
                                return dc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Exp), args.clone());
                            }
                            _ => {}
                        }
                    }
                }
                residual_integrate(dc, expr, var)
            }
            ApplicationHead::Extension(_) => residual_integrate(dc, expr, var),
        },
    }
}

fn residual_integrate(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> TermId {
    dc.apply_semantic(SemanticOperator::Integrate, vec![expr, dc.symbol_id(var)])
}

fn is_integrate_residual(dc: &DomainExecutionContext<'_>, value: TermId) -> bool {
    matches!(
        dc.application_head(value),
        Some((ApplicationHead::Semantic(SemanticOperator::Integrate), _))
    )
}

/// 积分并包装为 [`CalculusResult`]（初等 vs 未求值）。
pub fn integrate_checked(dc: &mut DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> CalculusResult<TermId> {
    let value = integrate(dc, expr, var);
    if is_integrate_residual(dc, value) {
        CalculusResult::Unevaluated { expression: value, reason: Diagnostic::new(DiagnosticCode::IntegralNotElementary) }
    }
    else {
        CalculusResult::Exact { value, conditions: Vec::new() }
    }
}

/// 经原函数求值 `F(upper) - F(lower)` 的定积分。
pub fn definite_integrate_checked(
    dc: &mut DomainExecutionContext<'_>,
    expr: TermId,
    var: SymbolId,
    lower: TermId,
    upper: TermId,
) -> CalculusResult<TermId> {
    let echo = |dc: &mut DomainExecutionContext<'_>| {
        let iter = dc.ordered(vec![dc.symbol_id(var), lower, upper]);
        dc.apply_semantic(SemanticOperator::Integrate, vec![expr, iter])
    };
    match integrate_checked(dc, expr, var) {
        CalculusResult::Exact { value: antideriv, conditions } => {
            let at_upper = dc.fold_term(replace_symbol(dc, antideriv, var, upper));
            let at_lower = dc.fold_term(replace_symbol(dc, antideriv, var, lower));
            if contains_symbol(dc, at_upper, var) || contains_symbol(dc, at_lower, var) {
                return CalculusResult::Unevaluated { expression: echo(dc), reason: Diagnostic::new(DiagnosticCode::IntegrationDomainInvalid) };
            }
            let neg = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), at_lower]);
            let value = dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![at_upper, neg]));
            CalculusResult::Exact { value, conditions }
        }
        CalculusResult::Conditional { value: antideriv, conditions } => {
            let at_upper = dc.fold_term(replace_symbol(dc, antideriv, var, upper));
            let at_lower = dc.fold_term(replace_symbol(dc, antideriv, var, lower));
            let neg = dc.apply_semantic(SemanticOperator::Multiply, vec![dc.in_(-1), at_lower]);
            let value = dc.fold_term(dc.apply_semantic(SemanticOperator::Add, vec![at_upper, neg]));
            CalculusResult::Conditional { value, conditions }
        }
        CalculusResult::Unevaluated { reason, .. } => CalculusResult::Unevaluated { expression: echo(dc), reason },
    }
}
