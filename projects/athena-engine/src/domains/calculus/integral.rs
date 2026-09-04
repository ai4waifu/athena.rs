//! 会话 arena 上的不定 / 定积分（初等子集 · `TermId` 进出）。

use athena_types::{Diagnostic, DiagnosticCode, TermId};

use super::{
    ctx::CalculusCtx,
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, replace_symbol},
};

/// 在 arena 上做符号积分（多项式 / 初等子集）。
pub fn integrate(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> TermId {
    let Some(shape) = cc.shape(expr)
    else {
        return expr;
    };
    match shape {
        crate::execution::vm::Shape::Number => {
            let n = cc.number_of(expr).map(|n| cc.copy(n)).expect("number");
            cc.ap("Times", vec![cc.num(n), cc.sym(var)])
        }
        crate::execution::vm::Shape::Str(_) | crate::execution::vm::Shape::Bool(_) | crate::execution::vm::Shape::Null => {
            cc.ap("Integrate", vec![expr, cc.sym(var)])
        }
        crate::execution::vm::Shape::Sym(s) => {
            if cc.sym_is(s, var) {
                let x2 = cc.ap("Power", vec![cc.sym(var), cc.in_(2)]);
                cc.eval(cc.ap("Divide", vec![x2, cc.in_(2)]))
            }
            else {
                cc.ap("Times", vec![expr, cc.sym(var)])
            }
        }
        crate::execution::vm::Shape::List(items) => {
            let iss = items.iter().map(|i| integrate(cc, *i, var)).collect();
            cc.list(iss)
        }
        crate::execution::vm::Shape::App(_, _) => {
            let Some((h, args)) = cc.app(expr)
            else {
                return expr;
            };
            match h.as_str() {
                "Plus" => {
                    let iss = args.iter().map(|a| integrate(cc, *a, var)).collect();
                    cc.eval(cc.ap("Plus", iss))
                }
                "Times" if args.len() == 2 => {
                    let (coeff, rest) = if cc.number_of(args[0]).is_some() {
                        (args[0], args[1])
                    }
                    else if cc.number_of(args[1]).is_some() {
                        (args[1], args[0])
                    }
                    else {
                        return cc.ap("Integrate", vec![expr, cc.sym(var)]);
                    };
                    let ir = integrate(cc, rest, var);
                    cc.eval(cc.ap("Times", vec![coeff, ir]))
                }
                "Power" if args.len() == 2 && cc.head_name(args[0]).is_some_and(|n| n == var) => {
                    if let Some(n) = cc.int_exp(args[1]) {
                        if n != -1 {
                            let p = cc.ap("Power", vec![args[0], cc.in_(n + 1)]);
                            return cc.eval(cc.ap("Divide", vec![p, cc.in_(n + 1)]));
                        }
                    }
                    cc.ap("Integrate", vec![expr, cc.sym(var)])
                }
                "Sin" if args.len() == 1 && cc.head_name(args[0]).is_some_and(|n| n == var) => {
                    let c = cc.ap("Cos", args.clone());
                    cc.eval(cc.ap("Times", vec![cc.in_(-1), c]))
                }
                "Cos" if args.len() == 1 && cc.head_name(args[0]).is_some_and(|n| n == var) => cc.ap("Sin", args.clone()),
                "Exp" if args.len() == 1 && cc.head_name(args[0]).is_some_and(|n| n == var) => cc.ap("Exp", args.clone()),
                _ => cc.ap("Integrate", vec![expr, cc.sym(var)]),
            }
        }
    }
}

/// 积分并包装为 [`CalculusResult`]（初等 vs 未求值）。
pub fn integrate_checked(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> CalculusResult<TermId> {
    let value = integrate(cc, expr, var);
    if cc.head_name(value).is_some_and(|h| h == "Integrate") {
        CalculusResult::Unevaluated { expression: value, reason: Diagnostic::new(DiagnosticCode::IntegralNotElementary) }
    }
    else {
        CalculusResult::Exact { value, conditions: Vec::new() }
    }
}

/// 经原函数求值 `F(upper) - F(lower)` 的定积分。
pub fn definite_integrate_checked(
    cc: &mut CalculusCtx<'_>,
    expr: TermId,
    var: &str,
    lower: TermId,
    upper: TermId,
) -> CalculusResult<TermId> {
    let echo = |cc: &mut CalculusCtx<'_>| {
        let iter = cc.list(vec![cc.sym(var), lower, upper]);
        cc.ap("Integrate", vec![expr, iter])
    };
    match integrate_checked(cc, expr, var) {
        CalculusResult::Exact { value: antideriv, conditions } => {
            let at_upper = cc.eval(replace_symbol(cc, antideriv, var, upper));
            let at_lower = cc.eval(replace_symbol(cc, antideriv, var, lower));
            if contains_symbol(cc, at_upper, var) || contains_symbol(cc, at_lower, var) {
                return CalculusResult::Unevaluated {
                    expression: echo(cc),
                    reason: Diagnostic::new(DiagnosticCode::IntegrationDomainInvalid),
                };
            }
            let neg = cc.ap("Times", vec![cc.in_(-1), at_lower]);
            let value = cc.eval(cc.ap("Plus", vec![at_upper, neg]));
            CalculusResult::Exact { value, conditions }
        }
        CalculusResult::Conditional { value: antideriv, conditions } => {
            let at_upper = cc.eval(replace_symbol(cc, antideriv, var, upper));
            let at_lower = cc.eval(replace_symbol(cc, antideriv, var, lower));
            let neg = cc.ap("Times", vec![cc.in_(-1), at_lower]);
            let value = cc.eval(cc.ap("Plus", vec![at_upper, neg]));
            CalculusResult::Conditional { value, conditions }
        }
        CalculusResult::Unevaluated { reason, .. } => CalculusResult::Unevaluated { expression: echo(cc), reason },
    }
}
