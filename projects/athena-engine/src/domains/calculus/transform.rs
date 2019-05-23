//! 积分变换 — 带显式 ROC 的 Laplace / Fourier / Z 引导实现（arena 版 · Living `25`）。

use athena_numeric::{Number, abs as num_abs, compare as num_compare};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, TermId};

use super::{ctx::CalculusCtx, request::TransformKind, result::CalculusResult};
use crate::execution::vm::Shape;

/// 收敛域 — 每个变换结果都必须携带。
#[derive(Debug, PartialEq)]
pub struct RegionOfConvergence {
    /// 已知时的结构化 / 桥接谓词（如 `Greater[Re[s], a]`）。
    pub predicate: Option<TermId>,
    /// ROC 是否已知（false ⇒ 不得假装绝对收敛）。
    pub known: bool,
}

impl RegionOfConvergence {
    /// 已知半平面 `Re[s] > a`（实数 `a`）。
    pub fn re_s_greater(cc: &mut CalculusCtx<'_>, s: &str, a: Number) -> Self {
        let re = cc.ap("Re", vec![cc.sym(s)]);
        let greater = cc.ap("Greater", vec![re, cc.num(a)]);
        Self { predicate: Some(greater), known: true }
    }

    /// Fourier 频率在实轴上（经典 L¹ / Schwartz 像）。
    pub fn real_line(cc: &mut CalculusCtx<'_>, omega: &str) -> Self {
        let element = cc.ap("Element", vec![cc.sym(omega), cc.sym("Reals")]);
        Self { predicate: Some(element), known: true }
    }

    /// Z 变换外半径 `Abs[z] > r`。
    pub fn abs_z_greater(cc: &mut CalculusCtx<'_>, z: &str, r: Number) -> Self {
        let abs = cc.ap("Abs", vec![cc.sym(z)]);
        let greater = cc.ap("Greater", vec![abs, cc.num(r)]);
        Self { predicate: Some(greater), known: true }
    }

    /// 全平面收敛（如 `KroneckerDelta[n]`）。
    pub fn entire_plane(cc: &mut CalculusCtx<'_>, z: &str) -> Self {
        let element = cc.ap("Element", vec![cc.sym(z), cc.sym("Complexes")]);
        Self { predicate: Some(element), known: true }
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
    pub expression: TermId,
    /// 时间 / 序列变量。
    pub time_variable: String,
    /// 变换变量（`s`、`ω`、`z` 等）。
    pub transform_variable: String,
    /// 收敛域。
    pub region_of_convergence: RegionOfConvergence,
}

impl TransformResult {
    /// 桥接形态 `LaplaceTransform[F, {t,s}, ROC]`。
    pub fn materialize_expression(&self, cc: &mut CalculusCtx<'_>) -> TermId {
        let vars = cc.list(vec![cc.sym(&self.time_variable), cc.sym(&self.transform_variable)]);
        let mut args = vec![self.expression, vars];
        if let Some(roc) = self.region_of_convergence.predicate {
            args.push(roc);
        }
        else {
            args.push(cc.sym("ROCUnknown"));
        }
        let head = match self.kind {
            TransformKind::Laplace => "LaplaceTransform",
            TransformKind::Fourier => "FourierTransform",
            TransformKind::Z => "ZTransform",
        };
        cc.ap(head, args)
    }
}

/// 已解码表达式的单边 Laplace 变换。
pub fn laplace_checked(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    time_variable: &str,
    transform_variable: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TransformResult> {
    match laplace_one(cc, expression, time_variable, transform_variable) {
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
                expression: echo_transform(
                    cc,
                    TransformKind::Laplace,
                    "LaplaceTransform",
                    expression,
                    time_variable,
                    transform_variable,
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
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    time_variable: &str,
    transform_variable: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TransformResult> {
    match fourier_one(cc, expression, time_variable, transform_variable) {
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
                expression: echo_transform(
                    cc,
                    TransformKind::Fourier,
                    "FourierTransform",
                    expression,
                    time_variable,
                    transform_variable,
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
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    time_variable: &str,
    transform_variable: &str,
    _assumptions: &AssumptionSet,
) -> CalculusResult<TransformResult> {
    match z_one(cc, expression, time_variable, transform_variable) {
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
                expression: echo_transform(cc, TransformKind::Z, "ZTransform", expression, time_variable, transform_variable),
                time_variable: time_variable.to_string(),
                transform_variable: transform_variable.to_string(),
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    }
}

fn echo_transform(
    cc: &mut CalculusCtx<'_>,
    _kind: TransformKind,
    head: &str,
    expression: TermId,
    time_variable: &str,
    transform_variable: &str,
) -> TermId {
    cc.ap(head, vec![expression, cc.sym(time_variable), cc.sym(transform_variable)])
}

fn laplace_one(cc: &mut CalculusCtx<'_>, expr: TermId, t: &str, s: &str) -> Option<(TermId, RegionOfConvergence)> {
    if let Some(n) = cc.number_of(expr).map(|n| cc.copy(n)) {
        // Laplace：ℒ{c} = c/s，Re(s)>0
        let sinv = cc.ap("Power", vec![cc.sym(s), cc.in_(-1)]);
        let body = cc.eval(cc.ap("Times", vec![cc.num(n), sinv]));
        return Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0))));
    }
    if is_sym_named(cc, expr, t) {
        // Laplace：ℒ{t} = 1/s²
        let body = cc.ap("Power", vec![cc.sym(s), cc.in_(-2)]);
        return Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0))));
    }
    let (h, args) = cc.app(expr)?;
    match h.as_str() {
        "Plus" => {
            let mut parts = Vec::new();
            let mut roc_bound = Number::small_int(0);
            for a in args {
                let (fa, roc) = laplace_one(cc, a, t, s)?;
                if let Some(b) = roc_half_plane_bound(cc, &roc) {
                    if num_compare(&b, &roc_bound) == Some(std::cmp::Ordering::Greater) {
                        roc_bound = b;
                    }
                }
                else if !roc.known {
                    return None;
                }
                parts.push(fa);
            }
            let body = if parts.len() == 1 { parts[0] } else { cc.eval(cc.ap("Plus", parts)) };
            Some((body, RegionOfConvergence::re_s_greater(cc, s, roc_bound)))
        }
        "Times" if args.len() == 2 => {
            if let Some(c) = cc.number_of(args[0]).map(|n| cc.copy(n)) {
                let (inner, roc) = laplace_one(cc, args[1], t, s)?;
                let body = cc.eval(cc.ap("Times", vec![cc.num(c), inner]));
                return Some((body, roc));
            }
            if let Some(c) = cc.number_of(args[1]).map(|n| cc.copy(n)) {
                let (inner, roc) = laplace_one(cc, args[0], t, s)?;
                let body = cc.eval(cc.ap("Times", vec![cc.num(c), inner]));
                return Some((body, roc));
            }
            None
        }
        "Power" if args.len() == 2 && is_sym_named(cc, args[0], t) => {
            let n = cc.int_exp(args[1])?;
            if n < 0 {
                return None;
            }
            let n_u = u32::try_from(n).ok()?;
            // Laplace：ℒ{tⁿ} = n!/sⁿ⁺¹
            let fact = factorial_u32(n_u)?;
            let spow = cc.ap("Power", vec![cc.sym(s), cc.in_(-(n_u as i64 + 1))]);
            let body = cc.eval(cc.ap("Times", vec![cc.in_(fact), spow]));
            Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0))))
        }
        "Exp" if args.len() == 1 => {
            // 形态：Exp[a t] 或 Exp[Times[a,t]]
            let a = match_coeff_times_var(cc, args[0], t)?;
            // 1/(s-a), Re(s)>a（实数 a）
            let neg = cc.ap("Times", vec![cc.in_(-1), cc.num(cc.copy(&a))]);
            let plus = cc.ap("Plus", vec![cc.sym(s), neg]);
            let body = cc.eval(cc.ap("Power", vec![plus, cc.in_(-1)]));
            Some((body, RegionOfConvergence::re_s_greater(cc, s, a)))
        }
        "Sin" if args.len() == 1 => {
            let w = match_coeff_times_var(cc, args[0], t)?;
            // Laplace：w/(s²+w²)
            let s2 = cc.ap("Power", vec![cc.sym(s), cc.in_(2)]);
            let w2 = cc.ap("Power", vec![cc.num(cc.copy(&w)), cc.in_(2)]);
            let den = cc.eval(cc.ap("Plus", vec![s2, w2]));
            let dinv = cc.ap("Power", vec![den, cc.in_(-1)]);
            let body = cc.eval(cc.ap("Times", vec![cc.num(w), dinv]));
            Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0))))
        }
        "Cos" if args.len() == 1 => {
            let w = match_coeff_times_var(cc, args[0], t)?;
            let s2 = cc.ap("Power", vec![cc.sym(s), cc.in_(2)]);
            let w2 = cc.ap("Power", vec![cc.num(cc.copy(&w)), cc.in_(2)]);
            let den = cc.eval(cc.ap("Plus", vec![s2, w2]));
            let dinv = cc.ap("Power", vec![den, cc.in_(-1)]);
            let body = cc.eval(cc.ap("Times", vec![cc.sym(s), dinv]));
            Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0))))
        }
        _ => None,
    }
}

fn fourier_one(cc: &mut CalculusCtx<'_>, expr: TermId, t: &str, omega: &str) -> Option<(TermId, RegionOfConvergence)> {
    let (h, args) = cc.app(expr)?;
    match h.as_str() {
        "Plus" => {
            let mut parts = Vec::new();
            for a in args {
                let (fa, roc) = fourier_one(cc, a, t, omega)?;
                if !roc.known {
                    return None;
                }
                parts.push(fa);
            }
            let body = if parts.len() == 1 { parts[0] } else { cc.eval(cc.ap("Plus", parts)) };
            Some((body, RegionOfConvergence::real_line(cc, omega)))
        }
        "Times" if args.len() == 2 => {
            if let Some(c) = cc.number_of(args[0]).map(|n| cc.copy(n)) {
                let (inner, roc) = fourier_one(cc, args[1], t, omega)?;
                let body = cc.eval(cc.ap("Times", vec![cc.num(c), inner]));
                return Some((body, roc));
            }
            if let Some(c) = cc.number_of(args[1]).map(|n| cc.copy(n)) {
                let (inner, roc) = fourier_one(cc, args[0], t, omega)?;
                let body = cc.eval(cc.ap("Times", vec![cc.num(c), inner]));
                return Some((body, roc));
            }
            // 形态：UnitStep[t] * Exp[-a t] → 1/(a + I ω)，a>0
            if let Some(rest) = split_unit_step(cc, &args, t) {
                return fourier_causal_exp(cc, rest, t, omega);
            }
            None
        }
        "Exp" if args.len() == 1 => {
            if let Some(a) = match_neg_coeff_abs_var(cc, args[0], t) {
                // 形态：Exp[-a Abs[t]] → 2a / (a² + ω²)，a>0
                if !number_is_positive(&a) {
                    return None;
                }
                let a2 = cc.ap("Power", vec![cc.num(cc.copy(&a)), cc.in_(2)]);
                let w2 = cc.ap("Power", vec![cc.sym(omega), cc.in_(2)]);
                let den = cc.eval(cc.ap("Plus", vec![a2, w2]));
                let dinv = cc.ap("Power", vec![den, cc.in_(-1)]);
                let two_a = cc.ap("Times", vec![cc.in_(2), cc.num(a)]);
                let body = cc.eval(cc.ap("Times", vec![two_a, dinv]));
                return Some((body, RegionOfConvergence::real_line(cc, omega)));
            }
            if let Some(a) = match_neg_coeff_square_var(cc, args[0], t) {
                // 形态：Exp[-a t²] → √(π/a) Exp[-ω²/(4a)]，a>0
                if !number_is_positive(&a) {
                    return None;
                }
                let ainv = cc.ap("Power", vec![cc.num(cc.copy(&a)), cc.in_(-1)]);
                let pia = cc.ap("Times", vec![cc.sym("Pi"), ainv]);
                let scale = cc.ap("Sqrt", vec![pia]);
                let w2 = cc.ap("Power", vec![cc.sym(omega), cc.in_(2)]);
                let four_a = cc.ap("Times", vec![cc.in_(4), cc.num(cc.copy(&a))]);
                let w24a = cc.ap("Power", vec![four_a, cc.in_(-1)]);
                let neg_w24a = cc.ap("Times", vec![cc.in_(-1), w24a]);
                let exp_arg = cc.eval(cc.ap("Times", vec![w2, neg_w24a]));
                let exp = cc.ap("Exp", vec![exp_arg]);
                let body = cc.eval(cc.ap("Times", vec![scale, exp]));
                return Some((body, RegionOfConvergence::real_line(cc, omega)));
            }
            None
        }
        _ => None,
    }
}

fn fourier_causal_exp(cc: &mut CalculusCtx<'_>, expr: TermId, t: &str, omega: &str) -> Option<(TermId, RegionOfConvergence)> {
    // 形态：Exp[-a t]（a>0）→ 1/(a + I ω)
    let (h, args) = cc.app(expr)?;
    if h != "Exp" || args.len() != 1 {
        return None;
    }
    let a_signed = match_coeff_times_var(cc, args[0], t)?;
    let zero = Number::small_int(0);
    if num_compare(&a_signed, &zero) != Some(std::cmp::Ordering::Less) {
        return None;
    }
    let a = evaluate_neg_number(cc, &a_signed)?;
    let iw = cc.ap("Times", vec![cc.sym("I"), cc.sym(omega)]);
    let den = cc.eval(cc.ap("Plus", vec![cc.num(a), iw]));
    let body = cc.eval(cc.ap("Power", vec![den, cc.in_(-1)]));
    Some((body, RegionOfConvergence::real_line(cc, omega)))
}

fn split_unit_step<'b>(cc: &CalculusCtx<'_>, args: &'b [TermId], t: &str) -> Option<TermId> {
    match args {
        [a, b] if is_unit_step(cc, *a, t) => Some(*b),
        [a, b] if is_unit_step(cc, *b, t) => Some(*a),
        _ => None,
    }
}

fn is_unit_step(cc: &CalculusCtx<'_>, term: TermId, t: &str) -> bool {
    let Some((h, args)) = cc.app(term)
    else {
        return false;
    };
    (h == "UnitStep" || h == "HeavisideTheta") && args.len() == 1 && is_sym_named(cc, args[0], t)
}

/// `Times[-a, Abs[t]]` 或等价，返回 a（要求最终为正衰减系数）。
fn match_neg_coeff_abs_var(cc: &mut CalculusCtx<'_>, term: TermId, var: &str) -> Option<Number> {
    let (h, args) = cc.app(term)?;
    if h != "Times" || args.len() != 2 {
        return None;
    }
    let coeff = if is_abs_of(cc, args[1], var) {
        cc.copy(cc.number_of(args[0])?)
    }
    else if is_abs_of(cc, args[0], var) {
        cc.copy(cc.number_of(args[1])?)
    }
    else {
        return None;
    };
    let zero = Number::small_int(0);
    if num_compare(&coeff, &zero) != Some(std::cmp::Ordering::Less) {
        return None;
    }
    evaluate_neg_number(cc, &coeff)
}

fn is_abs_of(cc: &CalculusCtx<'_>, term: TermId, var: &str) -> bool {
    let Some((h, args)) = cc.app(term)
    else {
        return false;
    };
    h == "Abs" && args.len() == 1 && is_sym_named(cc, args[0], var)
}

/// `Times[-a, Power[t, 2]]`，返回 a>0。
fn match_neg_coeff_square_var(cc: &mut CalculusCtx<'_>, term: TermId, var: &str) -> Option<Number> {
    let (h, args) = cc.app(term)?;
    if h != "Times" || args.len() != 2 {
        return None;
    }
    let coeff = if is_square_of(cc, args[1], var) {
        cc.copy(cc.number_of(args[0])?)
    }
    else if is_square_of(cc, args[0], var) {
        cc.copy(cc.number_of(args[1])?)
    }
    else {
        return None;
    };
    let zero = Number::small_int(0);
    if num_compare(&coeff, &zero) != Some(std::cmp::Ordering::Less) {
        return None;
    }
    evaluate_neg_number(cc, &coeff)
}

fn is_square_of(cc: &CalculusCtx<'_>, term: TermId, var: &str) -> bool {
    let Some((h, args)) = cc.app(term)
    else {
        return false;
    };
    h == "Power"
        && args.len() == 2
        && is_sym_named(cc, args[0], var)
        && cc.number_of(args[1]).and_then(|n| n.as_integer_exp()) == Some(2)
}

fn evaluate_neg_number(cc: &mut CalculusCtx<'_>, n: &Number) -> Option<Number> {
    let neg = cc.ap("Times", vec![cc.in_(-1), cc.num(cc.copy(n))]);
    let t = cc.eval(neg);
    cc.number_of(t).map(|v| cc.copy(v))
}

fn number_is_positive(n: &Number) -> bool {
    num_compare(n, &Number::small_int(0)) == Some(std::cmp::Ordering::Greater)
}

fn match_coeff_times_var(cc: &mut CalculusCtx<'_>, term: TermId, var: &str) -> Option<Number> {
    if is_sym_named(cc, term, var) {
        return Some(Number::small_int(1));
    }
    let (h, args) = cc.app(term)?;
    if h == "Times" && args.len() == 2 {
        if is_sym_named(cc, args[1], var) {
            return cc.number_of(args[0]).map(|n| cc.copy(n));
        }
        if is_sym_named(cc, args[0], var) {
            return cc.number_of(args[1]).map(|n| cc.copy(n));
        }
    }
    None
}

fn roc_half_plane_bound(cc: &mut CalculusCtx<'_>, roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate?;
    // 形态：Greater[Re[s], a]
    let (h, args) = cc.app(pred)?;
    if h == "Greater" && args.len() == 2 {
        return cc.number_of(args[1]).map(|n| cc.copy(n));
    }
    None
}

fn z_one(cc: &mut CalculusCtx<'_>, expr: TermId, n: &str, z: &str) -> Option<(TermId, RegionOfConvergence)> {
    if let Some(c) = cc.number_of(expr).map(|n| cc.copy(n)) {
        // Z 变换：c·u[n] → c·z/(z-1)，|z|>1
        let base = z_over_z_minus(cc, z, &Number::small_int(1));
        let body = cc.eval(cc.ap("Times", vec![cc.num(c), base]));
        return Some((body, RegionOfConvergence::abs_z_greater(cc, z, Number::small_int(1))));
    }
    if is_kronecker_delta(cc, expr, n) {
        return Some((cc.in_(1), RegionOfConvergence::entire_plane(cc, z)));
    }
    if is_unit_step(cc, expr, n) {
        return Some((
            z_over_z_minus(cc, z, &Number::small_int(1)),
            RegionOfConvergence::abs_z_greater(cc, z, Number::small_int(1)),
        ));
    }
    let (h, args) = cc.app(expr)?;
    match h.as_str() {
        "Plus" => {
            let mut parts = Vec::new();
            let mut radius = Number::small_int(0);
            let mut all_entire = true;
            for a in args {
                let (fa, roc) = z_one(cc, a, n, z)?;
                if let Some(r) = roc_abs_radius(cc, &roc) {
                    all_entire = false;
                    if num_compare(&r, &radius) == Some(std::cmp::Ordering::Greater) {
                        radius = r;
                    }
                }
                else if matches!(
                    roc.predicate,
                    Some(pred) if cc.app(pred).is_some_and(|(ph, _)| ph == "Element")
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
            let body = if parts.len() == 1 { parts[0] } else { cc.eval(cc.ap("Plus", parts)) };
            let roc = if all_entire {
                RegionOfConvergence::entire_plane(cc, z)
            }
            else {
                RegionOfConvergence::abs_z_greater(cc, z, radius)
            };
            Some((body, roc))
        }
        "Times" if args.len() == 2 => {
            if let Some(c) = cc.number_of(args[0]).map(|n| cc.copy(n)) {
                let (inner, roc) = z_one(cc, args[1], n, z)?;
                let body = cc.eval(cc.ap("Times", vec![cc.num(c), inner]));
                return Some((body, roc));
            }
            if let Some(c) = cc.number_of(args[1]).map(|n| cc.copy(n)) {
                let (inner, roc) = z_one(cc, args[0], n, z)?;
                let body = cc.eval(cc.ap("Times", vec![cc.num(c), inner]));
                return Some((body, roc));
            }
            // Z 变换：n·aⁿ → a·z/(z-a)²
            if let Some(a) = match_n_times_power(cc, &args, n) {
                let radius = num_abs(cc.copy(&a));
                let neg = cc.ap("Times", vec![cc.in_(-1), cc.num(cc.copy(&a))]);
                let za = cc.ap("Plus", vec![cc.sym(z), neg]);
                let den = cc.eval(cc.ap("Power", vec![za, cc.in_(2)]));
                let dinv = cc.ap("Power", vec![den, cc.in_(-1)]);
                let body = cc.eval(cc.ap("Times", vec![cc.num(a), cc.sym(z), dinv]));
                return Some((body, RegionOfConvergence::abs_z_greater(cc, z, radius)));
            }
            // 形态：UnitStep[n] * Power[a,n]
            if let Some(rest) = split_unit_step(cc, &args, n) {
                return z_one(cc, rest, n, z);
            }
            None
        }
        "Power" if args.len() == 2 && is_sym_named(cc, args[1], n) => {
            let a = cc.copy(cc.number_of(args[0])?);
            // Z 变换：aⁿ → z/(z-a)，|z|>|a|
            let radius = num_abs(cc.copy(&a));
            Some((z_over_z_minus(cc, z, &a), RegionOfConvergence::abs_z_greater(cc, z, radius)))
        }
        _ => None,
    }
}

fn z_over_z_minus(cc: &mut CalculusCtx<'_>, z: &str, a: &Number) -> TermId {
    let neg = cc.ap("Times", vec![cc.in_(-1), cc.num(cc.copy(a))]);
    let za = cc.ap("Plus", vec![cc.sym(z), neg]);
    let inv = cc.ap("Power", vec![za, cc.in_(-1)]);
    cc.eval(cc.ap("Times", vec![cc.sym(z), inv]))
}

fn is_kronecker_delta(cc: &CalculusCtx<'_>, term: TermId, n: &str) -> bool {
    let Some((h, args)) = cc.app(term)
    else {
        return false;
    };
    (h == "KroneckerDelta" || h == "DiscreteDelta") && args.len() == 1 && is_sym_named(cc, args[0], n)
}

fn match_n_times_power(cc: &CalculusCtx<'_>, args: &[TermId], n: &str) -> Option<Number> {
    match args {
        [a, b] if is_sym_named(cc, *a, n) => match_power_base(cc, *b, n),
        [a, b] if is_sym_named(cc, *b, n) => match_power_base(cc, *a, n),
        _ => None,
    }
}

fn match_power_base(cc: &CalculusCtx<'_>, term: TermId, n: &str) -> Option<Number> {
    let (h, args) = cc.app(term)?;
    if h == "Power" && args.len() == 2 && is_sym_named(cc, args[1], n) {
        return cc.number_of(args[0]).map(|n| cc.copy(n));
    }
    None
}

fn roc_abs_radius(cc: &mut CalculusCtx<'_>, roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate?;
    // 形态：Greater[Abs[z], r]
    let (h, args) = cc.app(pred)?;
    if h == "Greater" && args.len() == 2 {
        if let Some((ah, inner)) = cc.app(args[0]) {
            if ah == "Abs" && inner.len() == 1 {
                return cc.number_of(args[1]).map(|n| cc.copy(n));
            }
        }
    }
    None
}

fn is_sym_named(cc: &CalculusCtx<'_>, term: TermId, name: &str) -> bool {
    matches!(cc.shape(term), Some(Shape::Sym(s)) if cc.sym_is(s, name))
}

fn factorial_u32(n: u32) -> Option<i64> {
    let mut acc: i64 = 1;
    for k in 2..=n {
        acc = acc.checked_mul(k as i64)?;
    }
    Some(acc)
}
