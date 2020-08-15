//! 积分变换 — 带显式 ROC 的 Laplace / Fourier / Z 引导实现（arena 版 · ）。

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_numeric::{Number, abs as num_abs, compare as num_compare};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, Result, SymbolId, TermId};

use super::{request::TransformKind, result::CalculusResult, symbol_rewrite::is_symbol_id};
use crate::{domains::context::DomainExecutionContext, execution::shape::Shape};

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
    pub fn re_s_greater(cc: &mut DomainExecutionContext<'_>, s: SymbolId, a: Number) -> Self {
        Self::re_s_greater_term(cc, s, cc.num(a))
    }

    /// 已知半平面 `Re[s] > a`（`a` 可为符号项）。
    pub fn re_s_greater_term(cc: &mut DomainExecutionContext<'_>, s: SymbolId, a: TermId) -> Self {
        let re = cc.apply_semantic(SemanticOperator::RealPart, vec![cc.symbol_id(s)]);
        let greater = cc.apply_semantic(SemanticOperator::Greater, vec![re, a]);
        Self { predicate: Some(greater), known: true }
    }

    /// Fourier 频率在实轴上（经典 L¹ / Schwartz 像）。
    pub fn real_line(cc: &mut DomainExecutionContext<'_>, omega: SymbolId) -> Self {
        let element = cc.apply_semantic(SemanticOperator::MemberOf, vec![cc.symbol_id(omega), cc.symbol("Reals")]);
        Self { predicate: Some(element), known: true }
    }

    /// Z 变换外半径 `Abs[z] > r`。
    pub fn abs_z_greater(cc: &mut DomainExecutionContext<'_>, z: SymbolId, r: Number) -> Self {
        let abs = cc.apply_semantic(SemanticOperator::Abs, vec![cc.symbol_id(z)]);
        let greater = cc.apply_semantic(SemanticOperator::Greater, vec![abs, cc.num(r)]);
        Self { predicate: Some(greater), known: true }
    }

    /// 全平面收敛（如 `KroneckerDelta[n]`）。
    pub fn entire_plane(cc: &mut DomainExecutionContext<'_>, z: SymbolId) -> Self {
        let element = cc.apply_semantic(SemanticOperator::MemberOf, vec![cc.symbol_id(z), cc.symbol("Complexes")]);
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
    pub time_variable: SymbolId,
    /// 变换变量（`s`、`ω`、`z` 等）。
    pub transform_variable: SymbolId,
    /// 收敛域。
    pub region_of_convergence: RegionOfConvergence,
}

impl TransformResult {
    /// 桥接形态 `LaplaceTransform[F, {t,s}, ROC]`。
    pub fn materialize_expression(&self, cc: &mut DomainExecutionContext<'_>) -> TermId {
        let vars = cc.ordered(vec![cc.symbol_id(self.time_variable), cc.symbol_id(self.transform_variable)]);
        let mut args = vec![self.expression, vars];
        if let Some(roc) = self.region_of_convergence.predicate {
            args.push(roc);
        }
        else {
            args.push(cc.symbol("ROCUnknown"));
        }
        let op = match self.kind {
            TransformKind::Laplace => SemanticOperator::LaplaceTransform,
            TransformKind::Fourier => SemanticOperator::FourierTransform,
            TransformKind::Z => SemanticOperator::ZTransform,
        };
        cc.apply_semantic(op, args)
    }
}

/// 已解码表达式的单边 Laplace 变换。
pub fn laplace_checked(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    time_variable: SymbolId,
    transform_variable: SymbolId,
    _assumptions: &AssumptionSet,
) -> Result<CalculusResult<TransformResult>> {
    Ok(match laplace_one(cc, expression, time_variable, transform_variable)? {
        Some((expr, roc)) => CalculusResult::Exact {
            value: TransformResult {
                kind: TransformKind::Laplace,
                expression: expr,
                time_variable,
                transform_variable,
                region_of_convergence: roc,
            },
            conditions: Vec::new(),
        },
        None => CalculusResult::Unevaluated {
            expression: TransformResult {
                kind: TransformKind::Laplace,
                expression: echo_transform(cc, SemanticOperator::LaplaceTransform, expression, time_variable, transform_variable),
                time_variable,
                transform_variable,
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    })
}

/// Fourier 变换（非单位角频率约定 `∫ f(t) e^{-I ω t} dt`）。
///
/// 引导实现：双边指数衰减、Gaussian、因果指数，以及标量 / 加法线性组合。结果始终携带 ROC。
pub fn fourier_checked(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    time_variable: SymbolId,
    transform_variable: SymbolId,
    _assumptions: &AssumptionSet,
) -> Result<CalculusResult<TransformResult>> {
    Ok(match fourier_one(cc, expression, time_variable, transform_variable)? {
        Some((expr, roc)) => CalculusResult::Exact {
            value: TransformResult {
                kind: TransformKind::Fourier,
                expression: expr,
                time_variable,
                transform_variable,
                region_of_convergence: roc,
            },
            conditions: Vec::new(),
        },
        None => CalculusResult::Unevaluated {
            expression: TransformResult {
                kind: TransformKind::Fourier,
                expression: echo_transform(cc, SemanticOperator::FourierTransform, expression, time_variable, transform_variable),
                time_variable,
                transform_variable,
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    })
}

/// 单边 Z 变换 `X(z) = Σ_{n=0}^{∞} x[n] z^{-n}`。
///
/// 引导实现：`KroneckerDelta`、单位阶跃 / 常数、`a^n`、`n a^n`，以及标量 / 加法线性组合。结果始终携带 ROC。
pub fn z_checked(
    cc: &mut DomainExecutionContext<'_>,
    expression: TermId,
    time_variable: SymbolId,
    transform_variable: SymbolId,
    _assumptions: &AssumptionSet,
) -> Result<CalculusResult<TransformResult>> {
    Ok(match z_one(cc, expression, time_variable, transform_variable)? {
        Some((expr, roc)) => CalculusResult::Exact {
            value: TransformResult { kind: TransformKind::Z, expression: expr, time_variable, transform_variable, region_of_convergence: roc },
            conditions: Vec::new(),
        },
        None => CalculusResult::Unevaluated {
            expression: TransformResult {
                kind: TransformKind::Z,
                expression: echo_transform(cc, SemanticOperator::ZTransform, expression, time_variable, transform_variable),
                time_variable,
                transform_variable,
                region_of_convergence: RegionOfConvergence::unknown(),
            },
            reason: Diagnostic::new(DiagnosticCode::TransformRocUnknown),
        },
    })
}

fn echo_transform(
    cc: &mut DomainExecutionContext<'_>,
    op: SemanticOperator,
    expression: TermId,
    time_variable: SymbolId,
    transform_variable: SymbolId,
) -> TermId {
    cc.apply_semantic(op, vec![expression, cc.symbol_id(time_variable), cc.symbol_id(transform_variable)])
}

fn laplace_one(cc: &mut DomainExecutionContext<'_>, expr: TermId, t: SymbolId, s: SymbolId) -> Result<Option<(TermId, RegionOfConvergence)>> {
    if let Some(n) = cc.number_of(expr) {
        // 拉普拉斯：ℒ{c} = c/s，Re(s)>0
        let sinv = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(s), cc.in_(-1)]);
        let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(n), sinv]))?;
        return Ok(Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0)))));
    }
    if is_symbol_id(cc, expr, t) {
        // 拉普拉斯：ℒ{t} = 1/s²
        let body = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(s), cc.in_(-2)]);
        return Ok(Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0)))));
    }
    let Some((head, args)) = cc.application_head(expr) else { return Ok(None) };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Add) => {
            let mut parts = Vec::new();
            let mut roc_bound = Number::small_int(0);
            for a in args {
                let Some((fa, roc)) = laplace_one(cc, a, t, s)? else { return Ok(None) };
                if let Some(b) = roc_half_plane_bound(cc, &roc) {
                    if num_compare(&b, &roc_bound) == Some(std::cmp::Ordering::Greater) {
                        roc_bound = b;
                    }
                }
                else if !roc.known {
                    return Ok(None);
                }
                parts.push(fa);
            }
            let body = if parts.len() == 1 { parts[0] } else { cc.fold_term(cc.apply_semantic(SemanticOperator::Add, parts))? };
            Ok(Some((body, RegionOfConvergence::re_s_greater(cc, s, roc_bound))))
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
            if let Some(c) = cc.number_of(args[0]) {
                let Some((inner, roc)) = laplace_one(cc, args[1], t, s)? else { return Ok(None) };
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), inner]))?;
                return Ok(Some((body, roc)));
            }
            if let Some(c) = cc.number_of(args[1]) {
                let Some((inner, roc)) = laplace_one(cc, args[0], t, s)? else { return Ok(None) };
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), inner]))?;
                return Ok(Some((body, roc)));
            }
            Ok(None)
        }
        ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 && is_symbol_id(cc, args[0], t) => {
            let Some(n) = cc.int_exp(args[1]) else { return Ok(None) };
            if n < 0 {
                return Ok(None);
            }
            let Some(n_u) = u32::try_from(n).ok() else { return Ok(None) };
            // 拉普拉斯：ℒ{tⁿ} = n!/sⁿ⁺¹
            let Some(fact) = factorial_u32(n_u) else { return Ok(None) };
            let spow = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(s), cc.in_(-(n_u as i64 + 1))]);
            let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(fact), spow]))?;
            Ok(Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0)))))
        }
        ApplicationHead::Semantic(op) if op.as_unary() == Some(UnaryFunction::Exp) && args.len() == 1 => {
            // 形态：Exp[a t] / Exp[Times[a,t]] / Exp[Times[-1,a,t]]（`a` 可为符号）
            let Some(a) = match_term_coeff_times_var(cc, args[0], t) else { return Ok(None) };
            // 1/(s-a) = (s + (-a))^(-1)
            let neg_a = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), a]);
            let plus = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol_id(s), neg_a]);
            let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Power, vec![plus, cc.in_(-1)]))?;
            let roc = if let Some(n) = cc.number_of(a) {
                RegionOfConvergence::re_s_greater(cc, s, n)
            }
            else {
                RegionOfConvergence::re_s_greater_term(cc, s, a)
            };
            Ok(Some((body, roc)))
        }
        ApplicationHead::Semantic(op) if op.as_unary() == Some(UnaryFunction::Sin) && args.len() == 1 => {
            let Some(w) = match_coeff_times_var(cc, args[0], t) else { return Ok(None) };
            // 拉普拉斯：w/(s²+w²)
            let s2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(s), cc.in_(2)]);
            let w2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.num(cc.copy(&w)), cc.in_(2)]);
            let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![s2, w2]))?;
            let dinv = cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]);
            let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(w), dinv]))?;
            Ok(Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0)))))
        }
        ApplicationHead::Semantic(op) if op.as_unary() == Some(UnaryFunction::Cos) && args.len() == 1 => {
            let Some(w) = match_coeff_times_var(cc, args[0], t) else { return Ok(None) };
            let s2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(s), cc.in_(2)]);
            let w2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.num(cc.copy(&w)), cc.in_(2)]);
            let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![s2, w2]))?;
            let dinv = cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]);
            let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.symbol_id(s), dinv]))?;
            Ok(Some((body, RegionOfConvergence::re_s_greater(cc, s, Number::small_int(0)))))
        }
        _ => Ok(None),
    }
}

fn fourier_one(cc: &mut DomainExecutionContext<'_>, expr: TermId, t: SymbolId, omega: SymbolId) -> Result<Option<(TermId, RegionOfConvergence)>> {
    let Some((head, args)) = cc.application_head(expr) else { return Ok(None) };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Add) => {
            let mut parts = Vec::new();
            for a in args {
                let Some((fa, roc)) = fourier_one(cc, a, t, omega)? else { return Ok(None) };
                if !roc.known {
                    return Ok(None);
                }
                parts.push(fa);
            }
            let body = if parts.len() == 1 { parts[0] } else { cc.fold_term(cc.apply_semantic(SemanticOperator::Add, parts))? };
            Ok(Some((body, RegionOfConvergence::real_line(cc, omega))))
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
            if let Some(c) = cc.number_of(args[0]) {
                let Some((inner, roc)) = fourier_one(cc, args[1], t, omega)? else { return Ok(None) };
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), inner]))?;
                return Ok(Some((body, roc)));
            }
            if let Some(c) = cc.number_of(args[1]) {
                let Some((inner, roc)) = fourier_one(cc, args[0], t, omega)? else { return Ok(None) };
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), inner]))?;
                return Ok(Some((body, roc)));
            }
            // 形态：UnitStep[t] * Exp[-a t] → 1/(a + I ω)，a>0
            if let Some(rest) = split_unit_step(cc, &args, t) {
                return fourier_causal_exp(cc, rest, t, omega);
            }
            Ok(None)
        }
        ApplicationHead::Semantic(op) if op.as_unary() == Some(UnaryFunction::Exp) && args.len() == 1 => {
            if let Some(a) = match_neg_coeff_abs_var(cc, args[0], t)? {
                // 形态：Exp[-a Abs[t]] → 2a / (a² + ω²)，a>0
                if !number_is_positive(&a) {
                    return Ok(None);
                }
                let a2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.num(cc.copy(&a)), cc.in_(2)]);
                let w2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(omega), cc.in_(2)]);
                let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![a2, w2]))?;
                let dinv = cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]);
                let two_a = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(2), cc.num(a)]);
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![two_a, dinv]))?;
                return Ok(Some((body, RegionOfConvergence::real_line(cc, omega))));
            }
            if let Some(a) = match_neg_coeff_square_var(cc, args[0], t)? {
                // 形态：Exp[-a t²] → √(π/a) Exp[-ω²/(4a)]，a>0
                if !number_is_positive(&a) {
                    return Ok(None);
                }
                let ainv = cc.apply_semantic(SemanticOperator::Power, vec![cc.num(cc.copy(&a)), cc.in_(-1)]);
                let pia = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.math_constant(athena_ir::MathematicalConstant::Pi), ainv]);
                let scale = cc.apply_semantic(SemanticOperator::Sqrt, vec![pia]);
                let w2 = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol_id(omega), cc.in_(2)]);
                let four_a = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(4), cc.num(cc.copy(&a))]);
                let w24a = cc.apply_semantic(SemanticOperator::Power, vec![four_a, cc.in_(-1)]);
                let neg_w24a = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), w24a]);
                let exp_arg = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![w2, neg_w24a]))?;
                let exp = cc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Exp), vec![exp_arg]);
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![scale, exp]))?;
                return Ok(Some((body, RegionOfConvergence::real_line(cc, omega))));
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn fourier_causal_exp(
    cc: &mut DomainExecutionContext<'_>,
    expr: TermId,
    t: SymbolId,
    omega: SymbolId,
) -> Result<Option<(TermId, RegionOfConvergence)>> {
    // 形态：Exp[-a t]（a>0）→ 1/(a + I ω)
    let Some((head, args)) = cc.application_head(expr) else { return Ok(None) };
    if !matches!(head, ApplicationHead::Semantic(op) if op.as_unary() == Some(UnaryFunction::Exp)) || args.len() != 1 {
        return Ok(None);
    }
    let Some(a_signed) = match_coeff_times_var(cc, args[0], t) else { return Ok(None) };
    let zero = Number::small_int(0);
    if num_compare(&a_signed, &zero) != Some(std::cmp::Ordering::Less) {
        return Ok(None);
    }
    let Some(a) = evaluate_neg_number(cc, &a_signed)? else { return Ok(None) };
    let iw = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.symbol("I"), cc.symbol_id(omega)]);
    let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![cc.num(a), iw]))?;
    let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]))?;
    Ok(Some((body, RegionOfConvergence::real_line(cc, omega))))
}

fn split_unit_step<'b>(cc: &DomainExecutionContext<'_>, args: &'b [TermId], t: SymbolId) -> Option<TermId> {
    match args {
        [a, b] if is_unit_step(cc, *a, t) => Some(*b),
        [a, b] if is_unit_step(cc, *b, t) => Some(*a),
        _ => None,
    }
}

fn is_unit_step(cc: &DomainExecutionContext<'_>, term: TermId, t: SymbolId) -> bool {
    let Some((head, args)) = cc.application_head(term)
    else {
        return false;
    };
    cc.is_unit_step(head) && args.len() == 1 && is_symbol_id(cc, args[0], t)
}

/// `Times[-a, Abs[t]]` 或等价，返回 a（要求最终为正衰减系数）。
fn match_neg_coeff_abs_var(cc: &mut DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> Result<Option<Number>> {
    let Some((head, args)) = cc.application_head(term) else { return Ok(None) };
    if !matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) || args.len() != 2 {
        return Ok(None);
    }
    let coeff = if is_abs_of(cc, args[1], var) {
        let Some(n) = cc.number_of(args[0]) else { return Ok(None) };
        n
    }
    else if is_abs_of(cc, args[0], var) {
        let Some(n) = cc.number_of(args[1]) else { return Ok(None) };
        n
    }
    else {
        return Ok(None);
    };
    let zero = Number::small_int(0);
    if num_compare(&coeff, &zero) != Some(std::cmp::Ordering::Less) {
        return Ok(None);
    }
    evaluate_neg_number(cc, &coeff)
}

fn is_abs_of(cc: &DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> bool {
    matches!(
        cc.application_head(term),
        Some((ApplicationHead::Semantic(SemanticOperator::Abs), args))
            if args.len() == 1 && is_symbol_id(cc, args[0], var)
    )
}

/// `Times[-a, Power[t, 2]]`，返回 a>0。
fn match_neg_coeff_square_var(cc: &mut DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> Result<Option<Number>> {
    let Some((head, args)) = cc.application_head(term) else { return Ok(None) };
    if !matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) || args.len() != 2 {
        return Ok(None);
    }
    let coeff = if is_square_of(cc, args[1], var) {
        {
            let Some(n) = cc.number_of(args[0]) else { return Ok(None) };
            n
        }
    }
    else if is_square_of(cc, args[0], var) {
        {
            let Some(n) = cc.number_of(args[1]) else { return Ok(None) };
            n
        }
    }
    else {
        return Ok(None);
    };
    let zero = Number::small_int(0);
    if num_compare(&coeff, &zero) != Some(std::cmp::Ordering::Less) {
        return Ok(None);
    }
    evaluate_neg_number(cc, &coeff)
}

fn is_square_of(cc: &DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> bool {
    matches!(
        cc.application_head(term),
        Some((ApplicationHead::Semantic(SemanticOperator::Power), args))
            if args.len() == 2
                && is_symbol_id(cc, args[0], var)
                && cc.number_of(args[1]).and_then(|n| n.as_integer_exp()) == Some(2)
    )
}

fn evaluate_neg_number(cc: &mut DomainExecutionContext<'_>, n: &Number) -> Result<Option<Number>> {
    let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), cc.num(cc.copy(n))]);
    let t = cc.fold_term(neg)?;
    Ok(cc.number_of(t))
}

fn number_is_positive(n: &Number) -> bool {
    num_compare(n, &Number::small_int(0)) == Some(std::cmp::Ordering::Greater)
}

fn match_coeff_times_var(cc: &mut DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> Option<Number> {
    let coeff = match_term_coeff_times_var(cc, term, var)?;
    cc.number_of(coeff)
}

/// 匹配 `c * var`（含 n-ary `Times`），返回系数项 `c`（可为符号）。
fn match_term_coeff_times_var(cc: &DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> Option<TermId> {
    if is_symbol_id(cc, term, var) {
        return Some(cc.in_(1));
    }
    let Some((head, args)) = cc.application_head(term)
    else {
        return None;
    };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Negate) if args.len() == 1 => {
            let inner = match_term_coeff_times_var(cc, args[0], var)?;
            Some(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), inner]))
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) if !args.is_empty() => {
            let mut var_idx = None;
            for (i, a) in args.iter().enumerate() {
                if is_symbol_id(cc, *a, var) {
                    if var_idx.is_some() {
                        return None;
                    }
                    var_idx = Some(i);
                }
            }
            let idx = var_idx?;
            let rest: Vec<TermId> = args.iter().enumerate().filter(|(i, _)| *i != idx).map(|(_, a)| *a).collect();
            match rest.as_slice() {
                [] => Some(cc.in_(1)),
                [only] => Some(*only),
                many => Some(cc.apply_semantic(SemanticOperator::Multiply, many.to_vec())),
            }
        }
        _ => None,
    }
}

fn roc_half_plane_bound(cc: &mut DomainExecutionContext<'_>, roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate?;
    // 形态：Greater[Re[s], a]
    let (head, args) = cc.application_head(pred)?;
    if matches!(head, ApplicationHead::Semantic(SemanticOperator::Greater)) && args.len() == 2 {
        return cc.number_of(args[1]);
    }
    None
}

fn z_one(cc: &mut DomainExecutionContext<'_>, expr: TermId, n: SymbolId, z: SymbolId) -> Result<Option<(TermId, RegionOfConvergence)>> {
    if let Some(c) = cc.number_of(expr) {
        // Z 变换：c·u[n] → c·z/(z-1)，|z|>1
        let base = z_over_z_minus(cc, z, &Number::small_int(1))?;
        let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), base]))?;
        return Ok(Some((body, RegionOfConvergence::abs_z_greater(cc, z, Number::small_int(1)))));
    }
    if is_symbol_id(cc, expr, n) {
        // Z 变换：n → z/(z-1)²，|z|>1
        let zm1 = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol_id(z), cc.in_(-1)]);
        let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Power, vec![zm1, cc.in_(2)]))?;
        let dinv = cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]);
        let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.symbol_id(z), dinv]))?;
        return Ok(Some((body, RegionOfConvergence::abs_z_greater(cc, z, Number::small_int(1)))));
    }
    if is_kronecker_delta(cc, expr, n) {
        return Ok(Some((cc.in_(1), RegionOfConvergence::entire_plane(cc, z))));
    }
    if is_unit_step(cc, expr, n) {
        return Ok(Some((z_over_z_minus(cc, z, &Number::small_int(1))?, RegionOfConvergence::abs_z_greater(cc, z, Number::small_int(1)))));
    }
    let Some((head, args)) = cc.application_head(expr) else { return Ok(None) };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Add) => {
            let mut parts = Vec::new();
            let mut radius = Number::small_int(0);
            let mut all_entire = true;
            for a in args {
                let Some((fa, roc)) = z_one(cc, a, n, z)? else { return Ok(None) };
                if let Some(r) = roc_abs_radius(cc, &roc) {
                    all_entire = false;
                    if num_compare(&r, &radius) == Some(std::cmp::Ordering::Greater) {
                        radius = r;
                    }
                }
                else if matches!(
                    roc.predicate,
                    Some(pred) if cc.application_head(pred).is_some_and(|(ph, _)| cc.is_member_of(ph))
                ) {
                    // 整平面收敛 — 半径保持不变
                }
                else if !roc.known {
                    return Ok(None);
                }
                else {
                    all_entire = false;
                }
                parts.push(fa);
            }
            let body = if parts.len() == 1 { parts[0] } else { cc.fold_term(cc.apply_semantic(SemanticOperator::Add, parts))? };
            let roc = if all_entire { RegionOfConvergence::entire_plane(cc, z) } else { RegionOfConvergence::abs_z_greater(cc, z, radius) };
            Ok(Some((body, roc)))
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) if args.len() == 2 => {
            if let Some(c) = cc.number_of(args[0]) {
                let Some((inner, roc)) = z_one(cc, args[1], n, z)? else { return Ok(None) };
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), inner]))?;
                return Ok(Some((body, roc)));
            }
            if let Some(c) = cc.number_of(args[1]) {
                let Some((inner, roc)) = z_one(cc, args[0], n, z)? else { return Ok(None) };
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), inner]))?;
                return Ok(Some((body, roc)));
            }
            // Z 变换：n·aⁿ → a·z/(z-a)²
            if let Some(a) = match_n_times_power(cc, &args, n) {
                let radius = num_abs(cc.copy(&a));
                let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), cc.num(cc.copy(&a))]);
                let za = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol_id(z), neg]);
                let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Power, vec![za, cc.in_(2)]))?;
                let dinv = cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]);
                let body = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(a), cc.symbol_id(z), dinv]))?;
                return Ok(Some((body, RegionOfConvergence::abs_z_greater(cc, z, radius))));
            }
            // 形态：UnitStep[n] * Power[a,n]
            if let Some(rest) = split_unit_step(cc, &args, n) {
                return z_one(cc, rest, n, z);
            }
            Ok(None)
        }
        ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 && is_symbol_id(cc, args[1], n) => {
            let Some(a) = cc.number_of(args[0]) else { return Ok(None) };
            // Z 变换：aⁿ → z/(z-a)，|z|>|a|
            let radius = num_abs(cc.copy(&a));
            Ok(Some((z_over_z_minus(cc, z, &a)?, RegionOfConvergence::abs_z_greater(cc, z, radius))))
        }
        _ => Ok(None),
    }
}

fn z_over_z_minus(cc: &mut DomainExecutionContext<'_>, z: SymbolId, a: &Number) -> Result<TermId> {
    let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), cc.num(cc.copy(a))]);
    let za = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol_id(z), neg]);
    let inv = cc.apply_semantic(SemanticOperator::Power, vec![za, cc.in_(-1)]);
    cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.symbol_id(z), inv]))
}

fn is_kronecker_delta(cc: &DomainExecutionContext<'_>, term: TermId, n: SymbolId) -> bool {
    let Some((head, args)) = cc.application_head(term)
    else {
        return false;
    };
    cc.is_delta(head) && args.len() == 1 && is_symbol_id(cc, args[0], n)
}

fn match_n_times_power(cc: &DomainExecutionContext<'_>, args: &[TermId], n: SymbolId) -> Option<Number> {
    match args {
        [a, b] if is_symbol_id(cc, *a, n) => match_power_base(cc, *b, n),
        [a, b] if is_symbol_id(cc, *b, n) => match_power_base(cc, *a, n),
        _ => None,
    }
}

fn match_power_base(cc: &DomainExecutionContext<'_>, term: TermId, n: SymbolId) -> Option<Number> {
    let Some((head, args)) = cc.application_head(term) else { return None };
    if matches!(head, ApplicationHead::Semantic(SemanticOperator::Power)) && args.len() == 2 && is_symbol_id(cc, args[1], n) {
        return cc.number_of(args[0]);
    }
    None
}

fn roc_abs_radius(cc: &mut DomainExecutionContext<'_>, roc: &RegionOfConvergence) -> Option<Number> {
    let pred = roc.predicate?;
    // 形态：Greater[Abs[z], r]
    let (head, args) = cc.application_head(pred)?;
    if matches!(head, ApplicationHead::Semantic(SemanticOperator::Greater)) && args.len() == 2 {
        if let Some((ah, inner)) = cc.application_head(args[0]) {
            if matches!(ah, ApplicationHead::Semantic(SemanticOperator::Abs)) && inner.len() == 1 {
                return cc.number_of(args[1]);
            }
        }
    }
    None
}

fn factorial_u32(n: u32) -> Option<i64> {
    let mut acc: i64 = 1;
    for k in 2..=n {
        acc = acc.checked_mul(k as i64)?;
    }
    Some(acc)
}
