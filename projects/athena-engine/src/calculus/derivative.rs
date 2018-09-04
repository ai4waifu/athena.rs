//! 桥接 [`Term`] 上的求导。

use athena_types::{AssumptionSet, Predicate};

use crate::{
    eval::evaluate,
    function::lookup_function,
    term::{Atom, Term, number_from_term},
};

use super::result::{ConditionalResult, unresolved};

/// 在 `Term` 上做符号求导。
pub fn differentiate(expr: &Term, var: &str) -> Term {
    match expr {
        Term::Atom(Atom::Number(_)) | Term::Atom(Atom::String(_)) => Term::int(0),
        Term::Atom(Atom::Symbol(s)) if s == var => Term::int(1),
        Term::Atom(Atom::Symbol(_)) => Term::int(0),
        Term::List(items) => Term::List(items.iter().map(|i| differentiate(i, var)).collect()),
        Term::Application { head, arguments: args } => {
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
                    else if let Some(athena_types::Number::Real(athena_types::RealNumber::Machine(nf))) =
                        number_from_term(exp).cloned()
                    {
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
                // Abs / Sqrt：无条件路径保留 D；条件路径见 [`differentiate_checked`]。
                "Abs" | "Sqrt" if args.len() == 1 => Term::app("D", vec![expr.clone(), Term::symbol(var)]),
                _ => {
                    if let Some(def) = lookup_function(h) {
                        if def.arity == 1 && args.len() == 1 {
                            if let Some(df) = def.unary_derivative {
                                return evaluate(&Term::app(
                                    "Times",
                                    vec![df(&args[0]), differentiate(&args[0], var)],
                                ));
                            }
                        }
                    }
                    // 未知头部：保留 D，禁止静默当成 0。
                    Term::app("D", vec![expr.clone(), Term::symbol(var)])
                }
            }
        }
    }
}

/// 在假设下求导，返回条件而非裸项。
pub fn differentiate_checked(expr: &Term, var: &str, assumptions: &AssumptionSet) -> ConditionalResult<Term> {
    if let Term::Application { head, arguments: args } = expr {
        if head.is_symbol("Abs") && args.len() == 1 {
            let inner = &args[0];
            let candidate = evaluate(&Term::app(
                "Times",
                vec![
                    Term::app("Abs", vec![inner.clone()]),
                    Term::app("Power", vec![inner.clone(), Term::int(-1)]),
                    differentiate(inner, var),
                ],
            ));
            let needs_nonzero =
                !assumptions.predicates.iter().any(|p| matches!(p, Predicate::NonZero(_) | Predicate::SymbolNonZero(_)));
            if needs_nonzero {
                // TermId(0) 为桥接占位，直至 Abs 参数绑定落地。
                return ConditionalResult::with_unresolved(
                    candidate,
                    vec![unresolved(Predicate::NonZero(athena_types::TermId(0)))],
                );
            }
            return ConditionalResult::exact(candidate);
        }
        if head.is_symbol("Sqrt") && args.len() == 1 {
            let inner = &args[0];
            let candidate = evaluate(&Term::app(
                "Times",
                vec![
                    Term::app(
                        "Power",
                        vec![Term::app("Times", vec![Term::int(2), Term::app("Sqrt", vec![inner.clone()])]), Term::int(-1)],
                    ),
                    differentiate(inner, var),
                ],
            ));
            let needs_nonneg =
                !assumptions.predicates.iter().any(|p| matches!(p, Predicate::NonNegative(_) | Predicate::Positive(_)));
            if needs_nonneg {
                return ConditionalResult::with_unresolved(
                    candidate,
                    vec![unresolved(Predicate::NonNegative(athena_types::TermId(0)))],
                );
            }
            return ConditionalResult::exact(candidate);
        }
    }
    ConditionalResult::exact(evaluate(&differentiate(expr, var)))
}
