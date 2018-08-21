//! Indefinite integration on bridge [`Term`] (elementary subset).

use num_bigint::BigInt;
use num_traits::Zero;

use crate::eval::evaluate;
use crate::term::{Atom, Term, number_from_term};

use super::result::CalculusResult;

/// Symbolic integration on `Term` (polynomial / elementary subset).
pub fn integrate(expr: &Term, var: &str) -> Term {
    match expr {
        Term::Atom(Atom::Number(n)) => Term::app("Times", vec![Term::number(n.clone()), Term::symbol(var)]),
        Term::Atom(Atom::String(_)) => Term::app("Integrate", vec![expr.clone(), Term::symbol(var)]),
        Term::Atom(Atom::Symbol(s)) if s == var => evaluate(&Term::app(
            "Divide",
            vec![
                Term::app("Power", vec![Term::symbol(var), Term::int(2)]),
                Term::int(2),
            ],
        )),
        Term::Atom(Atom::Symbol(_)) => Term::app("Times", vec![expr.clone(), Term::symbol(var)]),
        Term::List(items) => Term::List(items.iter().map(|i| integrate(i, var)).collect()),
        Term::App { head, args } => {
            let h = head.head_name().unwrap_or("");
            match h {
                "Plus" => evaluate(&Term::app("Plus", args.iter().map(|a| integrate(a, var)).collect())),
                "Times" if args.len() == 2 => {
                    let (coeff, rest) = if number_from_term(&args[0]).is_some() {
                        (&args[0], &args[1])
                    } else if number_from_term(&args[1]).is_some() {
                        (&args[1], &args[0])
                    } else {
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

/// Integrate and wrap as [`CalculusResult`] (elementary vs unevaluated).
pub fn integrate_checked(expr: &Term, var: &str) -> CalculusResult<Term> {
    let value = integrate(expr, var);
    if matches!(&value, Term::App { head, .. } if head.is_symbol("Integrate")) {
        CalculusResult::Unevaluated {
            expression: value,
            reason: athena_types::Diagnostic::error(
                athena_types::DiagnosticCode::IntegralNotElementary,
                "no elementary antiderivative in current subset",
            ),
        }
    } else {
        CalculusResult::Exact {
            value,
            conditions: Vec::new(),
        }
    }
}
