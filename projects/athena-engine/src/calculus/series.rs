//! Series objects — Taylor bootstrap (about arbitrary finite center).

use athena_types::{Diagnostic, DiagnosticCode};

use crate::eval::evaluate;
use crate::term::{Term, number_from_term};

use super::derivative::differentiate;
use super::result::CalculusResult;
use super::term_util::{contains_symbol, replace_symbol};

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
    /// Power terms `(coefficient, power)` for `coeff * (variable - center)^power`.
    pub terms: Vec<(Term, i64)>,
    /// Truncation order (max power included).
    pub order: u32,
    /// Remainder.
    pub remainder: Remainder,
}

impl Series {
    /// Power of `(variable - center)`.
    fn delta_power(&self, power: i64) -> Term {
        let delta = if is_zero_term(&self.center) {
            Term::symbol(&self.variable)
        } else {
            evaluate(&Term::app(
                "Plus",
                vec![
                    Term::symbol(&self.variable),
                    Term::app("Times", vec![Term::int(-1), self.center.clone()]),
                ],
            ))
        };
        if power == 0 {
            Term::int(1)
        } else if power == 1 {
            delta
        } else {
            Term::app("Power", vec![delta, Term::integer(power)])
        }
    }

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
                } else {
                    evaluate(&Term::app("Times", vec![coeff.clone(), self.delta_power(*power)]))
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
    const SHIFT: &str = "__athena_taylor_t";
    let working = if is_zero_term(center) {
        expression.clone()
    } else {
        // f(x) about c  ≡  f(t + c) about t = 0.
        let shifted_var = evaluate(&Term::app("Plus", vec![Term::symbol(SHIFT), center.clone()]));
        replace_symbol(expression, variable, &shifted_var)
    };
    let expand_var = if is_zero_term(center) { variable } else { SHIFT };

    let mut terms = Vec::new();
    let mut current = working;
    let mut factorial: i64 = 1;
    for n in 0..=order {
        if n > 0 {
            factorial = factorial.saturating_mul(n as i64);
            current = evaluate(&differentiate(&current, expand_var));
        }
        let at_zero = evaluate(&replace_symbol(&current, expand_var, &Term::int(0)));
        if contains_symbol(&at_zero, expand_var) {
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

    let next = evaluate(&differentiate(&current, expand_var));
    let next_at = evaluate(&replace_symbol(&next, expand_var, &Term::int(0)));
    let remainder = if is_zero_term(&next_at) && !contains_symbol(&next, expand_var) {
        Remainder::ExactTruncation
    } else {
        let delta = if is_zero_term(center) {
            Term::symbol(variable)
        } else {
            evaluate(&Term::app(
                "Plus",
                vec![
                    Term::symbol(variable),
                    Term::app("Times", vec![Term::int(-1), center.clone()]),
                ],
            ))
        };
        Remainder::BigO(Term::app("Power", vec![delta, Term::int((order + 1) as i64)]))
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
