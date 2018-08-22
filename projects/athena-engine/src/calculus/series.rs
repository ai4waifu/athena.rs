//! Series objects — Taylor bootstrap for polynomial bridge terms.

use athena_types::{Diagnostic, DiagnosticCode};

use crate::eval::evaluate;
use crate::term::{Atom, Term, number_from_term};

use super::derivative::differentiate;
use super::result::CalculusResult;

/// Remainder annotation for truncated series.
#[derive(Debug, Clone, PartialEq)]
pub enum Remainder {
    /// Exact truncation (polynomial degree ≤ order).
    ExactTruncation,
    /// Big-O remainder term (expression).
    BigO(Term),
    /// Remainder unknown.
    Unknown,
}

/// Independent series value (not a bare polynomial list).
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// Expansion variable.
    pub variable: String,
    /// Center point (already decoded).
    pub center: Term,
    /// Power terms `(coefficient, power)` from low to high.
    pub terms: Vec<(Term, i64)>,
    /// Truncation order (max power included).
    pub order: u32,
    /// Remainder.
    pub remainder: Remainder,
}

impl Series {
    /// Convert to a Plus/Times/Power polynomial term when exact.
    pub fn to_term(&self) -> Term {
        if self.terms.is_empty() {
            return Term::int(0);
        }
        let parts: Vec<Term> = self
            .terms
            .iter()
            .map(|(coeff, power)| {
                if *power == 0 {
                    coeff.clone()
                } else if *power == 1 {
                    evaluate(&Term::app("Times", vec![coeff.clone(), Term::symbol(&self.variable)]))
                } else {
                    evaluate(&Term::app(
                        "Times",
                        vec![
                            coeff.clone(),
                            Term::app("Power", vec![Term::symbol(&self.variable), Term::integer(*power)]),
                        ],
                    ))
                }
            })
            .collect();
        if parts.len() == 1 {
            parts.into_iter().next().unwrap()
        } else {
            evaluate(&Term::app("Plus", parts))
        }
    }
}

fn residual_series(expression: &Term, variable: &str, center: &Term, order: u32) -> Series {
    Series {
        variable: variable.to_string(),
        center: center.clone(),
        terms: Vec::new(),
        order,
        remainder: Remainder::BigO(expression.clone()),
    }
}

/// Taylor expand about `center` up to `order` (inclusive power).
pub fn taylor(expression: &Term, variable: &str, center: &Term, order: u32) -> CalculusResult<Series> {
    if !is_zero_term(center) {
        return CalculusResult::Unevaluated {
            expression: residual_series(expression, variable, center, order),
            reason: Diagnostic::error(
                DiagnosticCode::UnsupportedOperation,
                "Taylor about non-zero center not implemented yet",
            ),
        };
    }

    let mut terms = Vec::new();
    let mut current = expression.clone();
    let mut factorial: i64 = 1;
    for n in 0..=order {
        if n > 0 {
            factorial = factorial.saturating_mul(n as i64);
            current = evaluate(&differentiate(&current, variable));
        }
        let at_zero = evaluate(&replace_symbol(&current, variable, &Term::int(0)));
        if contains_symbol(&at_zero, variable) {
            return CalculusResult::Unevaluated {
                expression: residual_series(expression, variable, center, order),
                reason: Diagnostic::error(
                    DiagnosticCode::SeriesRemainderUnknown,
                    "Taylor coefficient still depends on the variable",
                ),
            };
        }
        let coeff = if n == 0 || factorial == 1 {
            at_zero
        } else {
            evaluate(&Term::app("Divide", vec![at_zero, Term::int(factorial)]))
        };
        if !is_zero_term(&coeff) {
            terms.push((coeff, n as i64));
        }
    }

    let next = evaluate(&differentiate(&current, variable));
    let next_at = evaluate(&replace_symbol(&next, variable, &Term::int(0)));
    let remainder = if is_zero_term(&next_at) && !contains_symbol(&next, variable) {
        Remainder::ExactTruncation
    } else {
        Remainder::BigO(Term::app(
            "Power",
            vec![Term::symbol(variable), Term::int((order + 1) as i64)],
        ))
    };

    CalculusResult::Exact {
        value: Series {
            variable: variable.to_string(),
            center: center.clone(),
            terms,
            order,
            remainder,
        },
        conditions: Vec::new(),
    }
}

fn is_zero_term(expr: &Term) -> bool {
    number_from_term(expr).is_some_and(|n| n.is_zero())
}

fn replace_symbol(expr: &Term, var: &str, with: &Term) -> Term {
    match expr {
        Term::Atom(Atom::Symbol(s)) if s == var => with.clone(),
        Term::Atom(_) => expr.clone(),
        Term::List(items) => Term::List(items.iter().map(|i| replace_symbol(i, var, with)).collect()),
        Term::App { head, args } => Term::App {
            head: Box::new(replace_symbol(head, var, with)),
            args: args.iter().map(|a| replace_symbol(a, var, with)).collect(),
        },
    }
}

fn contains_symbol(expr: &Term, var: &str) -> bool {
    match expr {
        Term::Atom(Atom::Symbol(s)) => s == var,
        Term::Atom(_) => false,
        Term::List(items) => items.iter().any(|i| contains_symbol(i, var)),
        Term::App { head, args } => contains_symbol(head, var) || args.iter().any(|a| contains_symbol(a, var)),
    }
}
