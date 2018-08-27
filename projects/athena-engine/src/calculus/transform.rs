//! Integral transforms — Laplace bootstrap with explicit ROC.

use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, Number};

use crate::eval::evaluate;
use crate::term::{Atom, Term, number_from_term};

use super::request::TransformKind;
use super::result::CalculusResult;

/// Region of convergence — required on every transform result.
#[derive(Debug, Clone, PartialEq)]
pub struct RegionOfConvergence {
    /// Structured / bridge predicate when known (e.g. `Greater[Re[s], a]`).
    pub predicate: Option<Term>,
    /// Whether ROC is known (false ⇒ must not pretend absolute convergence).
    pub known: bool,
}

impl RegionOfConvergence {
    /// Known half-plane `Re[s] > a` for real `a`.
    pub fn re_s_greater(s: &str, a: Number) -> Self {
        Self {
            predicate: Some(Term::app(
                "Greater",
                vec![
                    Term::app("Re", vec![Term::symbol(s)]),
                    Term::number(a),
                ],
            )),
            known: true,
        }
    }

    /// ROC unknown — still attached, never omitted.
    pub fn unknown() -> Self {
        Self {
            predicate: None,
            known: false,
        }
    }
}

/// Transform result object (not a bare expression).
#[derive(Debug, Clone, PartialEq)]
pub struct TransformResult {
    /// Kind.
    pub kind: TransformKind,
    /// Transformed expression in the transform variable.
    pub expression: Term,
    /// Time / sequence variable.
    pub time_variable: String,
    /// Transform variable (`s`, `ω`, `z`, …).
    pub transform_variable: String,
    /// Region of convergence.
    pub region_of_convergence: RegionOfConvergence,
}

impl TransformResult {
    /// Bridge form `LaplaceTransform[F, {t,s}, ROC]` for hosts that need a Term.
    pub fn to_bridge_term(&self) -> Term {
        let mut args = vec![
            self.expression.clone(),
            Term::List(vec![
                Term::symbol(&self.time_variable),
                Term::symbol(&self.transform_variable),
            ]),
        ];
        if let Some(roc) = &self.region_of_convergence.predicate {
            args.push(roc.clone());
        } else {
            args.push(Term::symbol("ROCUnknown"));
        }
        let head = match self.kind {
            TransformKind::Laplace => "LaplaceTransform",
            TransformKind::Fourier => "FourierTransform",
            TransformKind::Z => "ZTransform",
        };
        Term::app(head, args)
    }
}

/// Unilateral Laplace transform of an already-decoded expression.
pub fn laplace_checked(
    expression: &Term,
    time_variable: &str,
    transform_variable: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TransformResult> {
    match laplace_one(expression, time_variable, transform_variable) {
        Some((expr, roc)) => CalculusResult::Exact {
            value: TransformResult {
                kind: TransformKind::Laplace,
                expression: expr,
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: roc,
            },
            conditions: Vec::new(),
        },
        None => CalculusResult::Unevaluated {
            expression: TransformResult {
                kind: TransformKind::Laplace,
                expression: Term::app(
                    "LaplaceTransform",
                    vec![
                        expression.clone(),
                        Term::symbol(time_variable),
                        Term::symbol(transform_variable),
                    ],
                ),
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::error(
                DiagnosticCode::TransformRocUnknown,
                "Laplace transform not in bootstrap table (poly/exp/sin/cos/linear)",
            ),
        },
    }
}

fn laplace_one(expr: &Term, t: &str, s: &str) -> Option<(Term, RegionOfConvergence)> {
    if let Some(n) = number_from_term(expr).cloned() {
        // L{c} = c/s, Re(s)>0
        let body = evaluate(&Term::app("Times", vec![Term::number(n), Term::app("Power", vec![Term::symbol(s), Term::int(-1)])]));
        return Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))));
    }
    if expr.is_symbol(t) {
        // L{t} = 1/s^2
        let body = Term::app("Power", vec![Term::symbol(s), Term::int(-2)]);
        return Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))));
    }
    match expr {
        Term::Application { head, arguments: args } => {
            let h = head.head_name()?;
            match h {
                "Plus" => {
                    let mut parts = Vec::new();
                    let mut roc_bound = Number::small_int(0);
                    for a in args {
                        let (fa, roc) = laplace_one(a, t, s)?;
                        if let Some(b) = roc_half_plane_bound(&roc) {
                            if b.compare(&roc_bound) == Some(std::cmp::Ordering::Greater) {
                                roc_bound = b;
                            }
                        } else if !roc.known {
                            return None;
                        }
                        parts.push(fa);
                    }
                    let body = if parts.len() == 1 {
                        parts.pop().unwrap()
                    } else {
                        evaluate(&Term::app("Plus", parts))
                    };
                    return Some((body, RegionOfConvergence::re_s_greater(s, roc_bound)));
                }
                "Times" if args.len() == 2 => {
                    if let Some(c) = number_from_term(&args[0]).cloned() {
                        let (inner, roc) = laplace_one(&args[1], t, s)?;
                        let body = evaluate(&Term::app("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    if let Some(c) = number_from_term(&args[1]).cloned() {
                        let (inner, roc) = laplace_one(&args[0], t, s)?;
                        let body = evaluate(&Term::app("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    None
                }
                "Power" if args.len() == 2 && args[0].is_symbol(t) => {
                    let n = number_from_term(&args[1]).and_then(|e| e.as_integer_exp())?;
                    if n < 0.into() {
                        return None;
                    }
                    let n_u = u32::try_from(&n).ok()?;
                    // L{t^n} = n! / s^{n+1}
                    let fact = factorial_u32(n_u)?;
                    let body = evaluate(&Term::app(
                        "Times",
                        vec![
                            Term::integer(fact),
                            Term::app("Power", vec![Term::symbol(s), Term::integer(-(n_u as i64 + 1))]),
                        ],
                    ));
                    Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))))
                }
                "Exp" if args.len() == 1 => {
                    // Exp[a t] or Exp[Times[a,t]]
                    let a = match_coeff_times_var(&args[0], t)?;
                    // 1/(s-a), Re(s)>a (for real a)
                    let body = evaluate(&Term::app(
                        "Power",
                        vec![
                            Term::app(
                                "Plus",
                                vec![Term::symbol(s), Term::app("Times", vec![Term::int(-1), Term::number(a.clone())])],
                            ),
                            Term::int(-1),
                        ],
                    ));
                    Some((body, RegionOfConvergence::re_s_greater(s, a)))
                }
                "Sin" if args.len() == 1 => {
                    let w = match_coeff_times_var(&args[0], t)?;
                    // w / (s^2 + w^2)
                    let den = evaluate(&Term::app(
                        "Plus",
                        vec![
                            Term::app("Power", vec![Term::symbol(s), Term::int(2)]),
                            Term::app("Power", vec![Term::number(w.clone()), Term::int(2)]),
                        ],
                    ));
                    let body = evaluate(&Term::app("Times", vec![Term::number(w), Term::app("Power", vec![den, Term::int(-1)])]));
                    Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))))
                }
                "Cos" if args.len() == 1 => {
                    let w = match_coeff_times_var(&args[0], t)?;
                    let den = evaluate(&Term::app(
                        "Plus",
                        vec![
                            Term::app("Power", vec![Term::symbol(s), Term::int(2)]),
                            Term::app("Power", vec![Term::number(w), Term::int(2)]),
                        ],
                    ));
                    let body = evaluate(&Term::app(
                        "Times",
                        vec![Term::symbol(s), Term::app("Power", vec![den, Term::int(-1)])],
                    ));
                    Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))))
                }
                _ => None,
            }
        }
        Term::Atom(Atom::Symbol(_)) => None,
        Term::List(_) => None,
        Term::Atom(_) => None,
    }
}

fn match_coeff_times_var(term: &Term, var: &str) -> Option<Number> {
    if term.is_symbol(var) {
        return Some(Number::small_int(1));
    }
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Times") && args.len() == 2 => {
            if args[1].is_symbol(var) {
                return number_from_term(&args[0]).cloned();
            }
            if args[0].is_symbol(var) {
                return number_from_term(&args[1]).cloned();
            }
            None
        }
        _ => None,
    }
}

fn roc_half_plane_bound(roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate.as_ref()?;
    // Greater[Re[s], a]
    match pred {
        Term::Application { head, arguments: args }
            if head.is_symbol("Greater") && args.len() == 2 =>
        {
            number_from_term(&args[1]).cloned()
        }
        _ => None,
    }
}

fn factorial_u32(n: u32) -> Option<i64> {
    let mut acc: i64 = 1;
    for k in 2..=n {
        acc = acc.checked_mul(k as i64)?;
    }
    Some(acc)
}
