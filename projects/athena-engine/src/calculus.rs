//! Differentiation and integration on bridge [`Term`].

use num_bigint::BigInt;
use num_traits::Zero;

use athena_types::{Number, RealNumber};

use crate::eval::evaluate;
use crate::term::{Atom, Term, number_from_term};

/// Symbolic differentiation on `Term`.
pub fn differentiate(expr: &Term, var: &str) -> Term {
    match expr {
        Term::Atom(Atom::Number(_)) | Term::Atom(Atom::String(_)) => Term::int(0),
        Term::Atom(Atom::Symbol(s)) if s == var => Term::int(1),
        Term::Atom(Atom::Symbol(_)) => Term::int(0),
        Term::List(items) => Term::List(items.iter().map(|i| differentiate(i, var)).collect()),
        Term::App { head, args } => {
            let h = head.head_name().unwrap_or("");
            match h {
                "Plus" => evaluate(&Term::app("Plus", args.iter().map(|a| differentiate(a, var)).collect())),
                "Times" => {
                    let mut terms = Vec::new();
                    for i in 0..args.len() {
                        let mut factors = args.clone();
                        factors[i] = differentiate(&args[i], var);
                        terms.push(Term::app("Times", factors));
                    }
                    evaluate(&Term::app("Plus", terms))
                }
                "Power" if args.len() == 2 => {
                    let base = &args[0];
                    let exp = &args[1];
                    if let Some(n) = number_from_term(exp).and_then(|e| e.as_integer_exp()) {
                        evaluate(&Term::app(
                            "Times",
                            vec![
                                Term::integer(n.clone()),
                                Term::app("Power", vec![base.clone(), Term::integer(n - 1i64)]),
                                differentiate(base, var),
                            ],
                        ))
                    }
                    else if let Some(Number::Real(RealNumber::Machine(nf))) = number_from_term(exp).cloned() {
                        evaluate(&Term::app(
                            "Times",
                            vec![
                                Term::real(nf),
                                Term::app("Power", vec![base.clone(), Term::real(nf - 1.0)]),
                                differentiate(base, var),
                            ],
                        ))
                    }
                    else {
                        Term::app("D", vec![expr.clone(), Term::symbol(var)])
                    }
                }
                "Sin" if args.len() == 1 => {
                    evaluate(&Term::app("Times", vec![Term::app("Cos", vec![args[0].clone()]), differentiate(&args[0], var)]))
                }
                "Cos" if args.len() == 1 => evaluate(&Term::app(
                    "Times",
                    vec![Term::int(-1), Term::app("Sin", vec![args[0].clone()]), differentiate(&args[0], var)],
                )),
                "Tan" if args.len() == 1 => evaluate(&Term::app(
                    "Times",
                    vec![
                        Term::app("Power", vec![Term::app("Cos", vec![args[0].clone()]), Term::int(-2)]),
                        differentiate(&args[0], var),
                    ],
                )),
                "Exp" if args.len() == 1 => {
                    evaluate(&Term::app("Times", vec![Term::app("Exp", vec![args[0].clone()]), differentiate(&args[0], var)]))
                }
                "Log" if args.len() == 1 => evaluate(&Term::app(
                    "Times",
                    vec![Term::app("Power", vec![args[0].clone(), Term::int(-1)]), differentiate(&args[0], var)],
                )),
                "Subtract" if args.len() == 2 => evaluate(&Term::app(
                    "Plus",
                    vec![differentiate(&args[0], var), Term::app("Times", vec![Term::int(-1), differentiate(&args[1], var)])],
                )),
                "Divide" if args.len() == 2 => {
                    let a = &args[0];
                    let b = &args[1];
                    evaluate(&Term::app(
                        "Times",
                        vec![
                            Term::app(
                                "Plus",
                                vec![
                                    Term::app("Times", vec![differentiate(a, var), b.clone()]),
                                    Term::app("Times", vec![Term::int(-1), a.clone(), differentiate(b, var)]),
                                ],
                            ),
                            Term::app("Power", vec![b.clone(), Term::int(-2)]),
                        ],
                    ))
                }
                _ => Term::int(0),
            }
        }
    }
}

/// Symbolic integration on `Term` (polynomial / elementary subset).
pub fn integrate(expr: &Term, var: &str) -> Term {
    match expr {
        Term::Atom(Atom::Number(n)) => Term::app("Times", vec![Term::number(n.clone()), Term::symbol(var)]),
        Term::Atom(Atom::String(_)) => Term::app("Integrate", vec![expr.clone(), Term::symbol(var)]),
        Term::Atom(Atom::Symbol(s)) if s == var => evaluate(&Term::app("Divide", vec![
            Term::app("Power", vec![Term::symbol(var), Term::int(2)]),
            Term::int(2),
        ])),
        Term::Atom(Atom::Symbol(_)) => Term::app("Times", vec![expr.clone(), Term::symbol(var)]),
        Term::List(items) => Term::List(items.iter().map(|i| integrate(i, var)).collect()),
        Term::App { head, args } => {
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
                            return evaluate(&Term::app("Divide", vec![
                                Term::app("Power", vec![args[0].clone(), Term::integer(n.clone() + 1i64)]),
                                Term::integer(n + 1i64),
                            ]));
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

