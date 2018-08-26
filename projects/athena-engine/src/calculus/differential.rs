//! Ordinary differential equations — first-order subset with residual verification.

use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, Number};

use crate::eval::evaluate;
use crate::term::{Atom, Term, number_from_term};

use super::derivative::differentiate;
use super::result::CalculusResult;
use super::term_util::replace_symbol;

/// Whether a candidate ODE solution was verified by residual substitution.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationStatus {
    /// Residual evaluates to zero.
    Verified {
        /// Residual expression after substitution (should be 0).
        residual: Term,
    },
    /// Residual did not reduce to zero.
    Failed {
        /// Non-zero residual.
        residual: Term,
    },
}

/// Explicit first-order ODE solution object (not a bare term).
#[derive(Debug, Clone, PartialEq)]
pub struct DifferentialSolution {
    /// Dependent variable name (bridge).
    pub dependent: String,
    /// Independent variable name.
    pub independent: String,
    /// Explicit particular solution right-hand side for `y(x)`.
    pub explicit: Term,
    /// Residual verification status — required for emitted solutions.
    pub verified: VerificationStatus,
}

impl DifferentialSolution {
    /// Bridge term `Equal[y[x], explicit]`.
    pub fn to_equal_term(&self) -> Term {
        Term::app(
            "Equal",
            vec![
                Term::app(self.dependent.as_str(), vec![Term::symbol(&self.independent)]),
                self.explicit.clone(),
            ],
        )
    }
}

/// Right-hand side of `y' = f(x, y)` after recognition.
struct FirstOrderRhs {
    /// `f` still possibly containing the dependent symbol.
    f: Term,
}

/// Solve a first-order ODE given as an already-decoded equation term.
///
/// Bootstrap forms:
/// - `Equal[D[y, x], a]` → particular `y = a x`
/// - `Equal[D[y, x], Times[a, y]]` → particular `y = Exp[a x]`
/// - `Equal[Plus[D[y, x], Times[p, y]], q]` (numeric `p≠0`) → particular `y = q/p`
pub fn solve_ode_checked(
    equation: &Term,
    dependent: &str,
    independent: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<DifferentialSolution> {
    let Some(rhs) = recognize_y_prime_equals(equation, dependent, independent) else {
        return unsupported(dependent, independent, equation);
    };

    let explicit = if let Some(a) = number_from_term(&rhs.f).cloned() {
        evaluate(&Term::app("Times", vec![Term::number(a), Term::symbol(independent)]))
    } else if let Some(a) = match_times_const_y(&rhs.f, dependent) {
        Term::app(
            "Exp",
            vec![evaluate(&Term::app(
                "Times",
                vec![Term::number(a), Term::symbol(independent)],
            ))],
        )
    } else if let Some((p, q)) = match_as_linear_forced(&rhs.f, dependent) {
        // y' = q - p y  came from y' + p y = q
        if p.is_zero() {
            return CalculusResult::Unevaluated {
                expression: placeholder(dependent, independent, equation.clone()),
                reason: Diagnostic::error(DiagnosticCode::OdeUnsupported, "linear ODE has zero damping"),
            };
        }
        evaluate(&Term::app("Divide", vec![Term::number(q), Term::number(p)]))
    } else {
        return unsupported(dependent, independent, equation);
    };

    let residual = residual_of(dependent, independent, &rhs.f, &explicit);
    if is_zero_term(&residual) {
        CalculusResult::Exact {
            value: DifferentialSolution {
                dependent: dependent.to_string(),
                independent: independent.to_string(),
                explicit,
                verified: VerificationStatus::Verified { residual },
            },
            conditions: Vec::new(),
        }
    } else {
        CalculusResult::Unevaluated {
            expression: DifferentialSolution {
                dependent: dependent.to_string(),
                independent: independent.to_string(),
                explicit,
                verified: VerificationStatus::Failed {
                    residual: residual.clone(),
                },
            },
            reason: Diagnostic::error(
                DiagnosticCode::OdeSolutionUnverified,
                format!("ODE residual did not vanish: {residual:?}"),
            ),
        }
    }
}

fn residual_of(dependent: &str, independent: &str, f: &Term, explicit: &Term) -> Term {
    let yp = evaluate(&differentiate(explicit, independent));
    let f_sub = evaluate(&replace_symbol(f, dependent, explicit));
    evaluate(&Term::app(
        "Plus",
        vec![yp, Term::app("Times", vec![Term::int(-1), f_sub])],
    ))
}

fn recognize_y_prime_equals(equation: &Term, dependent: &str, independent: &str) -> Option<FirstOrderRhs> {
    // Equal[D[y,x], rhs]
    if let Term::Application { head, arguments: args } = equation {
        if head.is_symbol("Equal") && args.len() == 2 && is_d_of(&args[0], dependent, independent) {
            return Some(FirstOrderRhs { f: args[1].clone() });
        }
        if head.is_symbol("Equal") && args.len() == 2 && is_d_of(&args[1], dependent, independent) {
            return Some(FirstOrderRhs { f: args[0].clone() });
        }
        // Equal[Plus[D[y,x], Times[p,y]], q]  ⇒  y' = q - p y
        if head.is_symbol("Equal") && args.len() == 2 {
            if let Some(p) = match_d_plus_p_y(&args[0], dependent, independent) {
                let q = number_from_term(&args[1]).cloned().unwrap_or_else(|| Number::small_int(0));
                let f = evaluate(&Term::app(
                    "Plus",
                    vec![
                        Term::number(q),
                        Term::app("Times", vec![Term::int(-1), Term::number(p), Term::symbol(dependent)]),
                    ],
                ));
                return Some(FirstOrderRhs { f });
            }
        }
    }
    None
}

fn match_d_plus_p_y(term: &Term, dependent: &str, independent: &str) -> Option<Number> {
    let Term::Application { head, arguments: args } = term else {
        return None;
    };
    if !head.is_symbol("Plus") || args.len() != 2 {
        return None;
    }
    if is_d_of(&args[0], dependent, independent) {
        return match_times_const_y(&args[1], dependent);
    }
    if is_d_of(&args[1], dependent, independent) {
        return match_times_const_y(&args[0], dependent);
    }
    None
}

fn match_as_linear_forced(f: &Term, dependent: &str) -> Option<(Number, Number)> {
    // f = q + Times[-1, p, y] or Plus[q, Times[-p, y]]
    match f {
        Term::Application { head, arguments: args } if head.is_symbol("Plus") && args.len() == 2 => {
            let (q_term, py_term) = if number_from_term(&args[0]).is_some() {
                (&args[0], &args[1])
            } else if number_from_term(&args[1]).is_some() {
                (&args[1], &args[0])
            } else {
                return None;
            };
            let q = number_from_term(q_term)?.clone();
            let Term::Application { head: th, arguments: targs } = py_term else {
                return None;
            };
            if !th.is_symbol("Times") {
                return None;
            }
            // Times[-1, p, y] or Times[-p, y]
            let mut coef = Number::small_int(1);
            let mut saw_y = false;
            for t in targs {
                if t.is_symbol(dependent) {
                    saw_y = true;
                } else if let Some(n) = number_from_term(t) {
                    coef = coef.mul(n.clone()).ok()?;
                } else {
                    return None;
                }
            }
            if !saw_y {
                return None;
            }
            // f = q + coef*y with coef = -p ⇒ p = -coef
            let p = coef.mul(Number::small_int(-1)).ok()?;
            Some((p, q))
        }
        _ => None,
    }
}

fn is_d_of(term: &Term, dependent: &str, independent: &str) -> bool {
    matches!(
        term,
        Term::Application { head, arguments: args }
            if head.is_symbol("D")
                && args.len() == 2
                && args[0].is_symbol(dependent)
                && args[1].is_symbol(independent)
    )
}

fn match_times_const_y(term: &Term, dependent: &str) -> Option<Number> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Times") && args.len() == 2 => {
            if args[1].is_symbol(dependent) {
                return number_from_term(&args[0]).cloned();
            }
            if args[0].is_symbol(dependent) {
                return number_from_term(&args[1]).cloned();
            }
            None
        }
        Term::Atom(Atom::Symbol(s)) if s == dependent => Some(Number::small_int(1)),
        _ => None,
    }
}

fn is_zero_term(expr: &Term) -> bool {
    number_from_term(expr).is_some_and(|n| n.is_zero())
}

fn placeholder(dependent: &str, independent: &str, equation: Term) -> DifferentialSolution {
    DifferentialSolution {
        dependent: dependent.to_string(),
        independent: independent.to_string(),
        explicit: equation,
        verified: VerificationStatus::Failed {
            residual: Term::symbol("Unevaluated"),
        },
    }
}

fn unsupported(dependent: &str, independent: &str, equation: &Term) -> CalculusResult<DifferentialSolution> {
    CalculusResult::Unevaluated {
        expression: placeholder(dependent, independent, equation.clone()),
        reason: Diagnostic::error(
            DiagnosticCode::OdeUnsupported,
            "ODE class not in first-order bootstrap subset",
        ),
    }
}
