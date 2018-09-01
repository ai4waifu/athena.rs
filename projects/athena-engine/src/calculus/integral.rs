//! 桥接 [`Term`] 上的不定 / 定积分（初等子集）。

use num_bigint::BigInt;
use num_traits::Zero;

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    eval::evaluate,
    term::{Atom, Term, number_from_term},
};

use super::{
    result::CalculusResult,
    term_util::{contains_symbol, replace_symbol},
};

/// 在 `Term` 上做符号积分（多项式 / 初等子集）。
pub fn integrate(expr: &Term, var: &str) -> Term {
    match expr {
        Term::Atom(Atom::Number(n)) => Term::app("Times", vec![Term::number(n.clone()), Term::symbol(var)]),
        Term::Atom(Atom::String(_)) => Term::app("Integrate", vec![expr.clone(), Term::symbol(var)]),
        Term::Atom(Atom::Symbol(s)) if s == var => {
            evaluate(&Term::app("Divide", vec![Term::app("Power", vec![Term::symbol(var), Term::int(2)]), Term::int(2)]))
        }
        Term::Atom(Atom::Symbol(_)) => Term::app("Times", vec![expr.clone(), Term::symbol(var)]),
        Term::List(items) => Term::List(items.iter().map(|i| integrate(i, var)).collect()),
        Term::Application { head, arguments: args } => {
            let h = head.head_name().unwrap_or("");
            match h {
                "Plus" => evaluate(&Term::app("Plus", args.iter().map(|a| integrate(a, var)).collect())),
                "Times" if args.len() == 2 => {
                    let (coeff, rest) = if number_from_term(&args[0]).is_some() {
                        (&args[0], &args[1])
                    }
                    else if number_from_term(&args[1]).is_some() {
                        (&args[1], &args[0])
                    }
                    else {
                        return Term::app("Integrate", vec![expr.clone(), Term::symbol(var)]);
                    };
                    evaluate(&Term::app("Times", vec![coeff.clone(), integrate(rest, var)]))
                }
                "Power" if args.len() == 2 && args[0].is_symbol(var) => {
                    if let Some(n) = number_from_term(&args[1]).and_then(|e| e.as_integer_exp()) {
                        if (n.clone() + 1) != BigInt::zero() {
                            return evaluate(&Term::app(
                                "Divide",
                                vec![
                                    Term::app("Power", vec![args[0].clone(), Term::integer(n.clone() + 1i64)]),
                                    Term::integer(n + 1i64),
                                ],
                            ));
                        }
                    }
                    Term::app("Integrate", vec![expr.clone(), Term::symbol(var)])
                }
                "Sin" if args.len() == 1 && args[0].is_symbol(var) => {
                    evaluate(&Term::app("Times", vec![Term::int(-1), Term::app("Cos", args.clone())]))
                }
                "Cos" if args.len() == 1 && args[0].is_symbol(var) => Term::app("Sin", args.clone()),
                "Exp" if args.len() == 1 && args[0].is_symbol(var) => Term::app("Exp", args.clone()),
                _ => Term::app("Integrate", vec![expr.clone(), Term::symbol(var)]),
            }
        }
    }
}

/// 积分并包装为 [`CalculusResult`]（初等 vs 未求值）。
pub fn integrate_checked(expr: &Term, var: &str) -> CalculusResult<Term> {
    let value = integrate(expr, var);
    if matches!(&value, Term::Application { head, .. } if head.is_symbol("Integrate")) {
        CalculusResult::Unevaluated {
            expression: value,
            reason: Diagnostic::error(DiagnosticCode::IntegralNotElementary, "当前子集无初等原函数"),
        }
    }
    else {
        CalculusResult::Exact { value, conditions: Vec::new() }
    }
}

/// 经原函数求值 `F(upper) - F(lower)` 的定积分。
pub fn definite_integrate_checked(expr: &Term, var: &str, lower: &Term, upper: &Term) -> CalculusResult<Term> {
    match integrate_checked(expr, var) {
        CalculusResult::Exact { value: antideriv, conditions } => {
            let at_upper = evaluate(&replace_symbol(&antideriv, var, upper));
            let at_lower = evaluate(&replace_symbol(&antideriv, var, lower));
            if contains_symbol(&at_upper, var) || contains_symbol(&at_lower, var) {
                return CalculusResult::Unevaluated {
                    expression: Term::app(
                        "Integrate",
                        vec![expr.clone(), Term::List(vec![Term::symbol(var), lower.clone(), upper.clone()])],
                    ),
                    reason: Diagnostic::error(
                        DiagnosticCode::IntegrationDomainInvalid,
                        "定积分上下限仍含自由变量",
                    ),
                };
            }
            let value = evaluate(&Term::app("Plus", vec![at_upper, Term::app("Times", vec![Term::int(-1), at_lower])]));
            CalculusResult::Exact { value, conditions }
        }
        CalculusResult::Conditional { value: antideriv, conditions } => {
            let at_upper = evaluate(&replace_symbol(&antideriv, var, upper));
            let at_lower = evaluate(&replace_symbol(&antideriv, var, lower));
            let value = evaluate(&Term::app("Plus", vec![at_upper, Term::app("Times", vec![Term::int(-1), at_lower])]));
            CalculusResult::Conditional { value, conditions }
        }
        CalculusResult::Unevaluated { reason, .. } => CalculusResult::Unevaluated {
            expression: Term::app(
                "Integrate",
                vec![expr.clone(), Term::List(vec![Term::symbol(var), lower.clone(), upper.clone()])],
            ),
            reason,
        },
    }
}
