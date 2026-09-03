//! 积分变换 — 带显式 ROC 的 Laplace / Fourier / Z 引导实现。

use athena_numeric::{Number, abs as num_abs, compare as num_compare};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode};

use crate::numeric_clone::{clone_number, clone_term};
use crate::{
    eval::evaluate,
    term::{Atom, Term, number_from_term},
};

use super::{request::TransformKind, result::CalculusResult};

/// 收敛域 — 每个变换结果都必须携带。
#[derive(Debug, PartialEq)]
pub struct RegionOfConvergence {
    /// 已知时的结构化 / 桥接谓词（如 `Greater[Re[s], a]`）。
    pub predicate: Option<Term>,
    /// ROC 是否已知（false ⇒ 不得假装绝对收敛）。
    pub known: bool,
}

impl RegionOfConvergence {
    /// 已知半平面 `Re[s] > a`（实数 `a`）。
    pub fn re_s_greater(s: &str, a: Number) -> Self {
        Self {
            predicate: Some(Term::apply("Greater", vec![Term::apply("Re", vec![Term::symbol(s)]), Term::number(a)])),
            known: true,
        }
    }

    /// Fourier 频率在实轴上（经典 L¹ / Schwartz 像）。
    pub fn real_line(omega: &str) -> Self {
        Self { predicate: Some(Term::apply("Element", vec![Term::symbol(omega), Term::symbol("Reals")])), known: true }
    }

    /// Z 变换外半径 `Abs[z] > r`。
    pub fn abs_z_greater(z: &str, r: Number) -> Self {
        Self {
            predicate: Some(Term::apply("Greater", vec![Term::apply("Abs", vec![Term::symbol(z)]), Term::number(r)])),
            known: true,
        }
    }

    /// 全平面收敛（如 `KroneckerDelta[n]`）。
    pub fn entire_plane(z: &str) -> Self {
        Self { predicate: Some(Term::apply("Element", vec![Term::symbol(z), Term::symbol("Complexes")])), known: true }
    }

    /// ROC 未知 — 仍须附着，不可省略。
    pub fn unknown() -> Self {
        Self { predicate: None, known: false }
    }
}

/// 变换结果对象（非裸表达式）。
#[derive(Debug, PartialEq)]
pub struct TransformResult {
    /// 种类。
    pub kind: TransformKind,
    /// 变换变量下的像函数表达式。
    pub expression: Term,
    /// 时间 / 序列变量。
    pub time_variable: String,
    /// 变换变量（`s`、`ω`、`z` 等）。
    pub transform_variable: String,
    /// 收敛域。
    pub region_of_convergence: RegionOfConvergence,
}

impl TransformResult {
    /// 桥接形态 `LaplaceTransform[F, {t,s}, ROC]`，供仍需要 Term 的宿主。
    pub fn to_bridge_term(&self) -> Term {
        let mut args = vec![
            clone_term(&self.expression),
            Term::List(vec![Term::symbol(&self.time_variable), Term::symbol(&self.transform_variable)]),
        ];
        if let Some(roc) = &self.region_of_convergence.predicate {
            args.push(clone_term(roc));
        }
        else {
            args.push(Term::symbol("ROCUnknown"));
        }
        let head = match self.kind {
            TransformKind::Laplace => "LaplaceTransform",
            TransformKind::Fourier => "FourierTransform",
            TransformKind::Z => "ZTransform",
        };
        Term::apply(head, args)
    }
}

/// 已解码表达式的单边 Laplace 变换。
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
                expression: Term::apply(
                    "LaplaceTransform",
                    vec![clone_term(&expression), Term::symbol(time_variable), Term::symbol(transform_variable)],
                ),
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    }
}

/// Fourier 变换（非单位角频率约定 `∫ f(t) e^{-I ω t} dt`）。
///
/// 引导实现：双边指数衰减、Gaussian、因果指数，以及标量 / 加法线性组合。结果始终携带 ROC。
pub fn fourier_checked(
    expression: &Term,
    time_variable: &str,
    transform_variable: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TransformResult> {
    match fourier_one(expression, time_variable, transform_variable) {
        Some((expr, roc)) => CalculusResult::Exact {
            value: TransformResult {
                kind: TransformKind::Fourier,
                expression: expr,
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: roc,
            },
            conditions: Vec::new(),
        },
        None => CalculusResult::Unevaluated {
            expression: TransformResult {
                kind: TransformKind::Fourier,
                expression: Term::apply(
                    "FourierTransform",
                    vec![clone_term(&expression), Term::symbol(time_variable), Term::symbol(transform_variable)],
                ),
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    }
}

/// 单边 Z 变换 `X(z) = Σ_{n=0}^{∞} x[n] z^{-n}`。
///
/// 引导实现：`KroneckerDelta`、单位阶跃 / 常数、`a^n`、`n a^n`，以及标量 / 加法线性组合。结果始终携带 ROC。
pub fn z_checked(
    expression: &Term,
    time_variable: &str,
    transform_variable: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TransformResult> {
    match z_one(expression, time_variable, transform_variable) {
        Some((expr, roc)) => CalculusResult::Exact {
            value: TransformResult {
                kind: TransformKind::Z,
                expression: expr,
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: roc,
            },
            conditions: Vec::new(),
        },
        None => CalculusResult::Unevaluated {
            expression: TransformResult {
                kind: TransformKind::Z,
                expression: Term::apply(
                    "ZTransform",
                    vec![clone_term(&expression), Term::symbol(time_variable), Term::symbol(transform_variable)],
                ),
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    }
}

fn laplace_one(expr: &Term, t: &str, s: &str) -> Option<(Term, RegionOfConvergence)> {
    if let Some(n) = number_from_term(expr).map(clone_number) {
        // Laplace：ℒ{c} = c/s，Re(s)>0
        let body =
            evaluate(&Term::apply("Times", vec![Term::number(n), Term::apply("Power", vec![Term::symbol(s), Term::int(-1)])]));
        return Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))));
    }
    if expr.is_symbol(t) {
        // Laplace：ℒ{t} = 1/s²
        let body = Term::apply("Power", vec![Term::symbol(s), Term::int(-2)]);
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
                            if num_compare(&b, &roc_bound) == Some(std::cmp::Ordering::Greater) {
                                roc_bound = b;
                            }
                        }
                        else if !roc.known {
                            return None;
                        }
                        parts.push(fa);
                    }
                    let body = if parts.len() == 1 { parts.pop().unwrap() } else { evaluate(&Term::apply("Plus", parts)) };
                    return Some((body, RegionOfConvergence::re_s_greater(s, roc_bound)));
                }
                "Times" if args.len() == 2 => {
                    if let Some(c) = number_from_term(&args[0]).map(clone_number) {
                        let (inner, roc) = laplace_one(&args[1], t, s)?;
                        let body = evaluate(&Term::apply("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    if let Some(c) = number_from_term(&args[1]).map(clone_number) {
                        let (inner, roc) = laplace_one(&args[0], t, s)?;
                        let body = evaluate(&Term::apply("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    None
                }
                "Power" if args.len() == 2 && args[0].is_symbol(t) => {
                    let n = number_from_term(&args[1]).and_then(|e| e.as_integer_exp())?;
                    if n < 0 {
                        return None;
                    }
                    let n_u = u32::try_from(n).ok()?;
                    // Laplace：ℒ{tⁿ} = n!/sⁿ⁺¹
                    let fact = factorial_u32(n_u)?;
                    let body = evaluate(&Term::apply(
                        "Times",
                        vec![
                            Term::integer(i64::from(fact)),
                            Term::apply("Power", vec![Term::symbol(s), Term::integer(-(n_u as i64 + 1))]),
                        ],
                    ));
                    Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))))
                }
                "Exp" if args.len() == 1 => {
                    // 形态：Exp[a t] 或 Exp[Times[a,t]]
                    let a = match_coeff_times_var(&args[0], t)?;
                    // 1/(s-a), Re(s)>a（实数 a）
                    let body = evaluate(&Term::apply(
                        "Power",
                        vec![
                            Term::apply(
                                "Plus",
                                vec![Term::symbol(s), Term::apply("Times", vec![Term::int(-1), Term::number(clone_number(&a))])],
                            ),
                            Term::int(-1),
                        ],
                    ));
                    Some((body, RegionOfConvergence::re_s_greater(s, a)))
                }
                "Sin" if args.len() == 1 => {
                    let w = match_coeff_times_var(&args[0], t)?;
                    // Laplace：w/(s²+w²)
                    let den = evaluate(&Term::apply(
                        "Plus",
                        vec![
                            Term::apply("Power", vec![Term::symbol(s), Term::int(2)]),
                            Term::apply("Power", vec![Term::number(clone_number(&w)), Term::int(2)]),
                        ],
                    ));
                    let body =
                        evaluate(&Term::apply("Times", vec![Term::number(w), Term::apply("Power", vec![den, Term::int(-1)])]));
                    Some((body, RegionOfConvergence::re_s_greater(s, Number::small_int(0))))
                }
                "Cos" if args.len() == 1 => {
                    let w = match_coeff_times_var(&args[0], t)?;
                    let den = evaluate(&Term::apply(
                        "Plus",
                        vec![
                            Term::apply("Power", vec![Term::symbol(s), Term::int(2)]),
                            Term::apply("Power", vec![Term::number(w), Term::int(2)]),
                        ],
                    ));
                    let body =
                        evaluate(&Term::apply("Times", vec![Term::symbol(s), Term::apply("Power", vec![den, Term::int(-1)])]));
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

fn fourier_one(expr: &Term, t: &str, omega: &str) -> Option<(Term, RegionOfConvergence)> {
    match expr {
        Term::Application { head, arguments: args } => {
            let h = head.head_name()?;
            match h {
                "Plus" => {
                    let mut parts = Vec::new();
                    for a in args {
                        let (fa, roc) = fourier_one(a, t, omega)?;
                        if !roc.known {
                            return None;
                        }
                        parts.push(fa);
                    }
                    let body = if parts.len() == 1 { parts.pop().unwrap() } else { evaluate(&Term::apply("Plus", parts)) };
                    Some((body, RegionOfConvergence::real_line(omega)))
                }
                "Times" if args.len() == 2 => {
                    if let Some(c) = number_from_term(&args[0]).map(clone_number) {
                        let (inner, roc) = fourier_one(&args[1], t, omega)?;
                        let body = evaluate(&Term::apply("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    if let Some(c) = number_from_term(&args[1]).map(clone_number) {
                        let (inner, roc) = fourier_one(&args[0], t, omega)?;
                        let body = evaluate(&Term::apply("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    // 形态：UnitStep[t] * Exp[-a t] → 1/(a + I ω)，a>0
                    if let Some(rest) = split_unit_step(args, t) {
                        return fourier_causal_exp(rest, t, omega);
                    }
                    None
                }
                "Exp" if args.len() == 1 => {
                    if let Some(a) = match_neg_coeff_abs_var(&args[0], t) {
                        // 形态：Exp[-a Abs[t]] → 2a / (a² + ω²)，a>0
                        if !number_is_positive(&a) {
                            return None;
                        }
                        let den = evaluate(&Term::apply(
                            "Plus",
                            vec![
                                Term::apply("Power", vec![Term::number(clone_number(&a)), Term::int(2)]),
                                Term::apply("Power", vec![Term::symbol(omega), Term::int(2)]),
                            ],
                        ));
                        let body = evaluate(&Term::apply(
                            "Times",
                            vec![
                                Term::apply("Times", vec![Term::int(2), Term::number(a)]),
                                Term::apply("Power", vec![den, Term::int(-1)]),
                            ],
                        ));
                        return Some((body, RegionOfConvergence::real_line(omega)));
                    }
                    if let Some(a) = match_neg_coeff_square_var(&args[0], t) {
                        // 形态：Exp[-a t²] → √(π/a) Exp[-ω²/(4a)]，a>0
                        if !number_is_positive(&a) {
                            return None;
                        }
                        let scale = Term::apply(
                            "Sqrt",
                            vec![Term::apply(
                                "Times",
                                vec![Term::symbol("Pi"), Term::apply("Power", vec![Term::number(clone_number(&a)), Term::int(-1)])],
                            )],
                        );
                        let exp_arg = evaluate(&Term::apply(
                            "Times",
                            vec![
                                Term::int(-1),
                                Term::apply(
                                    "Times",
                                    vec![
                                        Term::apply("Power", vec![Term::symbol(omega), Term::int(2)]),
                                        Term::apply(
                                            "Power",
                                            vec![Term::apply("Times", vec![Term::int(4), Term::number(a)]), Term::int(-1)],
                                        ),
                                    ],
                                ),
                            ],
                        ));
                        let body = evaluate(&Term::apply("Times", vec![scale, Term::apply("Exp", vec![exp_arg])]));
                        return Some((body, RegionOfConvergence::real_line(omega)));
                    }
                    None
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn fourier_causal_exp(expr: &Term, t: &str, omega: &str) -> Option<(Term, RegionOfConvergence)> {
    // 形态：Exp[-a t]（a>0）→ 1/(a + I ω)
    let Term::Application { head, arguments: args } = expr
    else {
        return None;
    };
    if !head.is_symbol("Exp") || args.len() != 1 {
        return None;
    }
    let a_signed = match_coeff_times_var(&args[0], t)?;
    let zero = Number::small_int(0);
    if num_compare(&a_signed, &zero) != Some(std::cmp::Ordering::Less) {
        return None;
    }
    let a = evaluate_neg_number(&a_signed)?;
    let den = evaluate(&Term::apply(
        "Plus",
        vec![Term::number(a), Term::apply("Times", vec![Term::symbol("I"), Term::symbol(omega)])],
    ));
    let body = evaluate(&Term::apply("Power", vec![den, Term::int(-1)]));
    Some((body, RegionOfConvergence::real_line(omega)))
}

fn split_unit_step<'a>(args: &'a [Term], t: &str) -> Option<&'a Term> {
    match args {
        [a, b] if is_unit_step(a, t) => Some(b),
        [a, b] if is_unit_step(b, t) => Some(a),
        _ => None,
    }
}

fn is_unit_step(term: &Term, t: &str) -> bool {
    matches!(
        term,
        Term::Application { head, arguments: args }
            if (head.is_symbol("UnitStep") || head.is_symbol("HeavisideTheta"))
                && args.len() == 1
                && args[0].is_symbol(t)
    )
}

/// `Times[-a, Abs[t]]` 或等价，返回 a（要求最终为正衰减系数）。
fn match_neg_coeff_abs_var(term: &Term, var: &str) -> Option<Number> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Times") && args.len() == 2 => {
            let coeff = if is_abs_of(&args[1], var) {
                number_from_term(&args[0]).map(clone_number)?
            }
            else if is_abs_of(&args[0], var) {
                number_from_term(&args[1]).map(clone_number)?
            }
            else {
                return None;
            };
            let zero = Number::small_int(0);
            if num_compare(&coeff, &zero) != Some(std::cmp::Ordering::Less) {
                return None;
            }
            evaluate_neg_number(&coeff)
        }
        _ => None,
    }
}

fn is_abs_of(term: &Term, var: &str) -> bool {
    matches!(
        term,
        Term::Application { head, arguments: args } if head.is_symbol("Abs") && args.len() == 1 && args[0].is_symbol(var)
    )
}

/// `Times[-a, Power[t, 2]]`，返回 a>0。
fn match_neg_coeff_square_var(term: &Term, var: &str) -> Option<Number> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Times") && args.len() == 2 => {
            let coeff = if is_square_of(&args[1], var) {
                number_from_term(&args[0]).map(clone_number)?
            }
            else if is_square_of(&args[0], var) {
                number_from_term(&args[1]).map(clone_number)?
            }
            else {
                return None;
            };
            let zero = Number::small_int(0);
            if num_compare(&coeff, &zero) != Some(std::cmp::Ordering::Less) {
                return None;
            }
            evaluate_neg_number(&coeff)
        }
        _ => None,
    }
}

fn is_square_of(term: &Term, var: &str) -> bool {
    matches!(
        term,
        Term::Application { head, arguments: args }
            if head.is_symbol("Power")
                && args.len() == 2
                && args[0].is_symbol(var)
                && number_from_term(&args[1]).and_then(|n| n.as_integer_exp()) == Some(2)
    )
}

fn evaluate_neg_number(n: &Number) -> Option<Number> {
    let t = evaluate(&Term::apply("Times", vec![Term::int(-1), Term::number(clone_number(&n))]));
    number_from_term(&t).map(clone_number)
}

fn number_is_positive(n: &Number) -> bool {
    num_compare(n, &Number::small_int(0)) == Some(std::cmp::Ordering::Greater)
}

fn match_coeff_times_var(term: &Term, var: &str) -> Option<Number> {
    if term.is_symbol(var) {
        return Some(Number::small_int(1));
    }
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Times") && args.len() == 2 => {
            if args[1].is_symbol(var) {
                return number_from_term(&args[0]).map(clone_number);
            }
            if args[0].is_symbol(var) {
                return number_from_term(&args[1]).map(clone_number);
            }
            None
        }
        _ => None,
    }
}

fn roc_half_plane_bound(roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate.as_ref()?;
    // 形态：Greater[Re[s], a]
    match pred {
        Term::Application { head, arguments: args } if head.is_symbol("Greater") && args.len() == 2 => {
            number_from_term(&args[1]).map(clone_number)
        }
        _ => None,
    }
}

fn z_one(expr: &Term, n: &str, z: &str) -> Option<(Term, RegionOfConvergence)> {
    if let Some(c) = number_from_term(expr).map(clone_number) {
        // Z 变换：c·u[n] → c·z/(z-1)，|z|>1
        let body = z_over_z_minus(z, &Number::small_int(1));
        let body = evaluate(&Term::apply("Times", vec![Term::number(c), body]));
        return Some((body, RegionOfConvergence::abs_z_greater(z, Number::small_int(1))));
    }
    if is_kronecker_delta(expr, n) {
        return Some((Term::int(1), RegionOfConvergence::entire_plane(z)));
    }
    if is_unit_step(expr, n) {
        return Some((z_over_z_minus(z, &Number::small_int(1)), RegionOfConvergence::abs_z_greater(z, Number::small_int(1))));
    }
    match expr {
        Term::Application { head, arguments: args } => {
            let h = head.head_name()?;
            match h {
                "Plus" => {
                    let mut parts = Vec::new();
                    let mut radius = Number::small_int(0);
                    let mut all_entire = true;
                    for a in args {
                        let (fa, roc) = z_one(a, n, z)?;
                        if let Some(r) = roc_abs_radius(&roc) {
                            all_entire = false;
                            if num_compare(&r, &radius) == Some(std::cmp::Ordering::Greater) {
                                radius = r;
                            }
                        }
                        else if matches!(
                            roc.predicate.as_ref(),
                            Some(Term::Application { head, .. }) if head.is_symbol("Element")
                        ) {
                            // 整平面收敛 — 半径保持不变
                        }
                        else if !roc.known {
                            return None;
                        }
                        else {
                            all_entire = false;
                        }
                        parts.push(fa);
                    }
                    let body = if parts.len() == 1 { parts.pop().unwrap() } else { evaluate(&Term::apply("Plus", parts)) };
                    let roc = if all_entire {
                        RegionOfConvergence::entire_plane(z)
                    }
                    else {
                        RegionOfConvergence::abs_z_greater(z, radius)
                    };
                    Some((body, roc))
                }
                "Times" if args.len() == 2 => {
                    if let Some(c) = number_from_term(&args[0]).map(clone_number) {
                        let (inner, roc) = z_one(&args[1], n, z)?;
                        let body = evaluate(&Term::apply("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    if let Some(c) = number_from_term(&args[1]).map(clone_number) {
                        let (inner, roc) = z_one(&args[0], n, z)?;
                        let body = evaluate(&Term::apply("Times", vec![Term::number(c), inner]));
                        return Some((body, roc));
                    }
                    // Z 变换：n·aⁿ → a·z/(z-a)²
                    if let Some(a) = match_n_times_power(args, n) {
                        let radius = num_abs(clone_number(&a));
                        let den = evaluate(&Term::apply(
                            "Power",
                            vec![
                                Term::apply(
                                    "Plus",
                                    vec![Term::symbol(z), Term::apply("Times", vec![Term::int(-1), Term::number(clone_number(&a))])],
                                ),
                                Term::int(2),
                            ],
                        ));
                        let body = evaluate(&Term::apply(
                            "Times",
                            vec![Term::number(a), Term::symbol(z), Term::apply("Power", vec![den, Term::int(-1)])],
                        ));
                        return Some((body, RegionOfConvergence::abs_z_greater(z, radius)));
                    }
                    // 形态：UnitStep[n] * Power[a,n]
                    if let Some(rest) = split_unit_step(args, n) {
                        return z_one(rest, n, z);
                    }
                    None
                }
                "Power" if args.len() == 2 && args[1].is_symbol(n) => {
                    let a = number_from_term(&args[0]).map(clone_number)?;
                    // Z 变换：aⁿ → z/(z-a)，|z|>|a|
                    let radius = num_abs(clone_number(&a));
                    Some((z_over_z_minus(z, &a), RegionOfConvergence::abs_z_greater(z, radius)))
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn z_over_z_minus(z: &str, a: &Number) -> Term {
    evaluate(&Term::apply(
        "Times",
        vec![
            Term::symbol(z),
            Term::apply(
                "Power",
                vec![
                    Term::apply(
                        "Plus",
                        vec![Term::symbol(z), Term::apply("Times", vec![Term::int(-1), Term::number(clone_number(&a))])],
                    ),
                    Term::int(-1),
                ],
            ),
        ],
    ))
}

fn is_kronecker_delta(term: &Term, n: &str) -> bool {
    matches!(
        term,
        Term::Application { head, arguments: args }
            if (head.is_symbol("KroneckerDelta") || head.is_symbol("DiscreteDelta"))
                && args.len() == 1
                && args[0].is_symbol(n)
    )
}

fn match_n_times_power(args: &[Term], n: &str) -> Option<Number> {
    match args {
        [a, b] if a.is_symbol(n) => match_power_base(b, n),
        [a, b] if b.is_symbol(n) => match_power_base(a, n),
        _ => None,
    }
}

fn match_power_base(term: &Term, n: &str) -> Option<Number> {
    match term {
        Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 && args[1].is_symbol(n) => {
            number_from_term(&args[0]).map(clone_number)
        }
        _ => None,
    }
}

fn roc_abs_radius(roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate.as_ref()?;
    // 形态：Greater[Abs[z], r]
    match pred {
        Term::Application { head, arguments: args }
            if head.is_symbol("Greater")
                && args.len() == 2
                && matches!(&args[0], Term::Application { head, arguments: inner } if head.is_symbol("Abs") && inner.len() == 1) =>
        {
            number_from_term(&args[1]).map(clone_number)
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
