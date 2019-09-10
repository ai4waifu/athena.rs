//! 会话 arena 上的不定 / 定积分（初等子集 · `TermId` 进出）。

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_types::{Diagnostic, DiagnosticCode, TermId};

use super::{
    ctx::CalculusCtx,
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, replace_symbol},
};
use crate::execution::shape::Shape;

/// 在 arena 上做符号积分（多项式 / 初等子集）。
pub fn integrate(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> TermId {
    let Some(shape) = cc.shape(expr)
    else {
        return expr;
    };
    match shape {
        Shape::Number => {
            let n = cc.number_of(expr).map(|n| cc.copy(n)).expect("number");
            cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(n), cc.symbol(var)])
        }
        Shape::String(_) | Shape::Bool(_) | Shape::Null => residual_integrate(cc, expr, var),
        Shape::Symbol(s) => {
            if cc.symbol_is(s, var) {
                let x2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol(var), cc.in_(2)]);
                cc.eval(cc.apply_semantic(SemanticOperator::Divide, vec![x2, cc.in_(2)]))
            }
            else {
                cc.apply_semantic(SemanticOperator::Multiply, vec![expr, cc.symbol(var)])
            }
        }
        Shape::Collection(items) => {
            let iss = items.iter().map(|i| integrate(cc, *i, var)).collect();
            cc.list(iss)
        }
        Shape::Application(head, args) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
                let iss = args.iter().map(|a| integrate(cc, *a, var)).collect();
                cc.eval(cc.apply_semantic(SemanticOperator::Add, iss))
            }
            ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
                let (coeff, rest) = if cc.number_of(args[0]).is_some() {
                    (args[0], args[1])
                }
                else if cc.number_of(args[1]).is_some() {
                    (args[1], args[0])
                }
                else {
                    return residual_integrate(cc, expr, var);
                };
                let ir = integrate(cc, rest, var);
                cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![coeff, ir]))
            }
            ApplicationHead::Semantic(SemanticOperator::Power)
                if args.len() == 2 && is_symbol_named(cc, args[0], var) =>
            {
                if let Some(n) = cc.int_exp(args[1]) {
                    if n != -1 {
                        let p = cc.apply_semantic(SemanticOperator::Power, vec![args[0], cc.in_(n + 1)]);
                        return cc.eval(cc.apply_semantic(SemanticOperator::Divide, vec![p, cc.in_(n + 1)]));
                    }
                }
                residual_integrate(cc, expr, var)
            }
            ApplicationHead::Semantic(op) => {
                if let Some(uf) = op.as_unary() {
                    if args.len() == 1 && is_symbol_named(cc, args[0], var) {
                        match uf {
                            UnaryFunction::Sin => {
                                let c = cc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Cos), args.clone());
                                return cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), c]));
                            }
                            UnaryFunction::Cos => {
                                return cc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Sin), args.clone());
                            }
                            UnaryFunction::Exp => {
                                return cc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Exp), args.clone());
                            }
                            _ => {}
                        }
                    }
                }
                residual_integrate(cc, expr, var)
            }
            ApplicationHead::Extension(_) => residual_integrate(cc, expr, var),
        },
    }
}

fn residual_integrate(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> TermId {
    cc.apply_semantic(SemanticOperator::Integrate, vec![expr, cc.symbol(var)])
}

fn is_integrate_residual(cc: &CalculusCtx<'_>, value: TermId) -> bool {
    matches!(
        cc.application_head(value),
        Some((ApplicationHead::Semantic(SemanticOperator::Integrate), _))
    )
}

fn is_symbol_named(cc: &CalculusCtx<'_>, term: TermId, name: &str) -> bool {
    matches!(cc.shape(term), Some(Shape::Symbol(s)) if cc.symbol_is(s, name))
}

/// 积分并包装为 [`CalculusResult`]（初等 vs 未求值）。
pub fn integrate_checked(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> CalculusResult<TermId> {
    let value = integrate(cc, expr, var);
    if is_integrate_residual(cc, value) {
        CalculusResult::Unevaluated { expression: value, reason: Diagnostic::new(DiagnosticCode::IntegralNotElementary) }
    }
    else {
        CalculusResult::Exact { value, conditions: Vec::new() }
    }
}

/// 经原函数求值 `F(upper) - F(lower)` 的定积分。
pub fn definite_integrate_checked(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str, lower: TermId, upper: TermId) -> CalculusResult<TermId> {
    let echo = |cc: &mut CalculusCtx<'_>| {
        let iter = cc.list(vec![cc.symbol(var), lower, upper]);
        cc.apply_semantic(SemanticOperator::Integrate, vec![expr, iter])
    };
    match integrate_checked(cc, expr, var) {
        CalculusResult::Exact { value: antideriv, conditions } => {
            let at_upper = cc.eval(replace_symbol(cc, antideriv, var, upper));
            let at_lower = cc.eval(replace_symbol(cc, antideriv, var, lower));
            if contains_symbol(cc, at_upper, var) || contains_symbol(cc, at_lower, var) {
                return CalculusResult::Unevaluated { expression: echo(cc), reason: Diagnostic::new(DiagnosticCode::IntegrationDomainInvalid) };
            }
            let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), at_lower]);
            let value = cc.eval(cc.apply_semantic(SemanticOperator::Add, vec![at_upper, neg]));
            CalculusResult::Exact { value, conditions }
        }
        CalculusResult::Conditional { value: antideriv, conditions } => {
            let at_upper = cc.eval(replace_symbol(cc, antideriv, var, upper));
            let at_lower = cc.eval(replace_symbol(cc, antideriv, var, lower));
            let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), at_lower]);
            let value = cc.eval(cc.apply_semantic(SemanticOperator::Add, vec![at_upper, neg]));
            CalculusResult::Conditional { value, conditions }
        }
        CalculusResult::Unevaluated { reason, .. } => CalculusResult::Unevaluated { expression: echo(cc), reason },
    }
}
