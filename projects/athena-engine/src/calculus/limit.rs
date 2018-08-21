//! Limit evaluation — finite substitution first; ∞ / sides later.

use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode};

use crate::eval::evaluate;
use crate::term::{Atom, Term, number_from_term};

use super::request::{LimitApproach, LimitDirection};
use super::result::CalculusResult;

/// Attempt a limit under assumptions.
pub fn limit_checked(
    expression: &Term,
    variable: &str,
    approach: &LimitApproach,
    direction: LimitDirection,
    _assumptions: &AssumptionSet,
) -> CalculusResult<Term> {
    match approach {
        LimitApproach::Finite(point) if direction == LimitDirection::TwoSided => {
            limit_finite_twosided(expression, variable, point)
        }
        _ => unevaluated_limit(expression, variable, approach, direction, "limit approach not implemented yet"),
    }
}

fn limit_finite_twosided(expression: &Term, variable: &str, point: &Term) -> CalculusResult<Term> {
    let substituted = replace_symbol(expression, variable, point);
    let value = evaluate(&substituted);
    if contains_symbol(&value, variable) {
        return unevaluated_limit(
            expression,
            variable,
            &LimitApproach::Finite(point.clone()),
            LimitDirection::TwoSided,
            "limit still depends on the variable after substitution",
        );
    }
    if is_indeterminate_form(&value) {
        return CalculusResult::Unevaluated {
            expression: Term::app(
                "Limit",
                vec![
                    expression.clone(),
                    Term::List(vec![Term::symbol(variable), point.clone()]),
                ],
            ),
            reason: Diagnostic::error(
                DiagnosticCode::LimitDoesNotExist,
                "indeterminate form after direct substitution",
            ),
        };
    }
    // Residual Limit / unevaluated specials → not closed.
    if matches!(&value, Term::App { head, .. } if head.is_symbol("Limit") || head.is_symbol("Indeterminate")) {
        return unevaluated_limit(
            expression,
            variable,
            &LimitApproach::Finite(point.clone()),
            LimitDirection::TwoSided,
            "limit did not reduce to a closed value",
        );
    }
    CalculusResult::Exact {
        value,
        conditions: Vec::new(),
    }
}

fn unevaluated_limit(
    expression: &Term,
    variable: &str,
    approach: &LimitApproach,
    direction: LimitDirection,
    detail: &str,
) -> CalculusResult<Term> {
    let approach_term = match approach {
        LimitApproach::Finite(t) => t.clone(),
        LimitApproach::PositiveInfinity => Term::symbol("Infinity"),
        LimitApproach::NegativeInfinity => Term::app("Times", vec![Term::int(-1), Term::symbol("Infinity")]),
    };
    let mut args = vec![expression.clone(), Term::List(vec![Term::symbol(variable), approach_term])];
    if direction != LimitDirection::TwoSided {
        args.push(Term::symbol(match direction {
            LimitDirection::FromBelow => "FromBelow",
            LimitDirection::FromAbove => "FromAbove",
            LimitDirection::TwoSided => unreachable!(),
        }));
    }
    CalculusResult::Unevaluated {
        expression: Term::app("Limit", args),
        reason: Diagnostic::error(DiagnosticCode::UnsupportedOperation, detail),
    }
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

fn is_indeterminate_form(expr: &Term) -> bool {
    // Direct 0/0 after eval often remains as Divide/Power with zeros.
    match expr {
        Term::App { head, args } if head.is_symbol("Divide") && args.len() == 2 => {
            number_from_term(&args[0]).is_some_and(|n| n.is_zero())
                && number_from_term(&args[1]).is_some_and(|n| n.is_zero())
        }
        Term::App { head, args } if head.is_symbol("Indeterminate") => true,
        _ => false,
    }
}
