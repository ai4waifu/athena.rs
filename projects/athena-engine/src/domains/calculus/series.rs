//! 级数对象 — Taylor / Laurent / 渐近（`x→∞`）引导实现（arena 版 · Living `25`）。

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_types::{Diagnostic, DiagnosticCode, TermId};

use super::{
    ctx::CalculusCtx,
    derivative::differentiate,
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, replace_symbol},
};
use crate::execution::shape::Shape;

/// 截断级数的余项标注。
#[derive(Debug, PartialEq)]
pub enum Remainder {
    /// 精确截断（多项式次数 ≤ order）。
    ExactTruncation,
    /// Big-O 余项（表达式）。
    BigO(TermId),
    /// Little-o 余项（表达式）。
    LittleO(TermId),
    /// 余项未知。
    Unknown,
}

/// 独立级数值（非裸多项式列表）。
#[derive(Debug, PartialEq)]
pub struct Series {
    /// 展开变量。
    pub variable: String,
    /// 展开中心（已解码；渐近于 ∞ 时为符号 `Infinity`）。
    pub center: TermId,
    /// 幂次项 `(coefficient, power)`：
    /// - 有限中心：`coeff * (variable - center)^power`
    /// - `Infinity`：系数形式 `coeff * variable^power`
    pub terms: Vec<(TermId, i64)>,
    /// 截断阶（有限中心：最高幂次；渐近：保留的 `t=1/x` 最高幂次）。
    pub order: u32,
    /// 余项。
    pub remainder: Remainder,
}

impl Series {
    /// 展开基幂：有限中心用 `(x-c)^p`，无穷用 `x^p`。
    fn delta_power(&self, cc: &mut CalculusCtx<'_>, power: i64) -> TermId {
        if self.center_is_infinity(cc) {
            if power == 0 {
                return cc.in_(1);
            }
            if power == 1 {
                return cc.symbol(&self.variable);
            }
            return cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol(&self.variable), cc.in_(power)]);
        }
        let delta = if is_zero_term(cc, self.center) {
            cc.symbol(&self.variable)
        }
        else {
            let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), self.center]);
            let plus = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol(&self.variable), neg]);
            cc.eval(plus)
        };
        if power == 0 {
            cc.in_(1)
        }
        else if power == 1 {
            delta
        }
        else {
            cc.apply_semantic(SemanticOperator::Power, vec![delta, cc.in_(power)])
        }
    }

    fn center_is_infinity(&self, cc: &CalculusCtx<'_>) -> bool {
        matches!(cc.shape(self.center), Some(Shape::Symbol(s)) if cc.symbol_is(s, "Infinity"))
    }

    /// 精确时转为 Plus/Times/Power 多项式项。
    pub fn to_term(&self, cc: &mut CalculusCtx<'_>) -> TermId {
        if self.terms.is_empty() {
            return cc.in_(0);
        }
        let parts: Vec<TermId> = self
            .terms
            .iter()
            .map(|(coeff, power)| {
                if *power == 0 {
                    *coeff
                }
                else {
                    let dp = self.delta_power(cc, *power);
                    cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![*coeff, dp]))
                }
            })
            .collect();
        if parts.len() == 1 { parts[0] } else { cc.eval(cc.apply_semantic(SemanticOperator::Add, parts)) }
    }
}

fn residual_series(cc: &mut CalculusCtx<'_>, expression: TermId, variable: &str, center: TermId, order: u32) -> Series {
    Series { variable: variable.to_string(), center, terms: Vec::new(), order, remainder: Remainder::BigO(expression) }
}

/// 关于 `center` 展开到 `order`（含该幂次）的 Taylor 展开。
pub fn taylor(cc: &mut CalculusCtx<'_>, expression: TermId, variable: &str, center: TermId, order: u32) -> CalculusResult<Series> {
    const SHIFT: &str = "__athena_taylor_t";
    let working = if is_zero_term(cc, center) {
        expression
    }
    else {
        // f(x) 关于 c  ≡  f(t + c) 关于 t = 0。
        let shifted_var = {
            let plus = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol(SHIFT), center]);
            cc.eval(plus)
        };
        replace_symbol(cc, expression, variable, shifted_var)
    };
    let expand_var = if is_zero_term(cc, center) { variable } else { SHIFT };

    let mut terms = Vec::new();
    let mut current = working;
    let mut factorial: i64 = 1;
    for n in 0..=order {
        if n > 0 {
            factorial = factorial.saturating_mul(n as i64);
            let d = differentiate(cc, current, expand_var);
            current = cc.eval(d);
        }
        let zero = cc.in_(0);
        let at_zero = cc.eval(replace_symbol(cc, current, expand_var, zero));
        if contains_symbol(cc, at_zero, expand_var) {
            return CalculusResult::Unevaluated {
                expression: residual_series(cc, expression, variable, center, order),
                reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
            };
        }
        let coeff = if n == 0 || factorial == 1 { at_zero } else { cc.eval(cc.apply_semantic(SemanticOperator::Divide, vec![at_zero, cc.in_(factorial)])) };
        if !is_zero_term(cc, coeff) {
            terms.push((coeff, n as i64));
        }
    }

    let next = differentiate(cc, current, expand_var);
    let next = cc.eval(next);
    let zero = cc.in_(0);
    let next_at = cc.eval(replace_symbol(cc, next, expand_var, zero));
    let remainder = if is_zero_term(cc, next_at) && !contains_symbol(cc, next, expand_var) {
        Remainder::ExactTruncation
    }
    else {
        let delta = if is_zero_term(cc, center) {
            cc.symbol(variable)
        }
        else {
            let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), center]);
            let plus = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol(variable), neg]);
            cc.eval(plus)
        };
        let pow = cc.apply_semantic(SemanticOperator::Power, vec![delta, cc.in_((order + 1) as i64)]);
        Remainder::BigO(pow)
    };

    CalculusResult::Exact { value: Series { variable: variable.to_string(), center, terms, order, remainder }, conditions: Vec::new() }
}

/// 关于 `center` 的 Laurent 展开：先清除有限阶极点，再 Taylor，再平移幂次。
///
/// `order` 为正则部分（非负幂）截断的最高幂次。主部在可清除时完整保留。
pub fn laurent(cc: &mut CalculusCtx<'_>, expression: TermId, variable: &str, center: TermId, order: u32) -> CalculusResult<Series> {
    const MAX_POLE: u32 = 8;
    let delta = if is_zero_term(cc, center) {
        cc.symbol(variable)
    }
    else {
        let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), center]);
        let plus = cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol(variable), neg]);
        cc.eval(plus)
    };

    for m in 0..=MAX_POLE {
        let cleared = if m == 0 {
            expression
        }
        else {
            let dpow = cc.apply_semantic(SemanticOperator::Power, vec![delta, cc.in_(m as i64)]);
            let times = cc.apply_semantic(SemanticOperator::Multiply, vec![expression, dpow]);
            cc.eval(times)
        };
        match taylor(cc, cleared, variable, center, order.saturating_add(m)) {
            CalculusResult::Exact { value: series, conditions } => {
                if series.terms.iter().any(|(coeff, _)| term_has_singular_zero_power(cc, *coeff)) {
                    continue;
                }
                return CalculusResult::Exact { value: remap_laurent_series(cc, series, variable, center, order, m, delta), conditions };
            }
            CalculusResult::Conditional { value: series, conditions } => {
                if series.terms.iter().any(|(coeff, _)| term_has_singular_zero_power(cc, *coeff)) {
                    continue;
                }
                return CalculusResult::Conditional { value: remap_laurent_series(cc, series, variable, center, order, m, delta), conditions };
            }
            CalculusResult::Unevaluated { .. } => continue,
        }
    }

    CalculusResult::Unevaluated {
        expression: residual_series(cc, expression, variable, center, order),
        reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
    }
}

fn remap_laurent_series(cc: &mut CalculusCtx<'_>, series: Series, variable: &str, center: TermId, order: u32, m: u32, delta: TermId) -> Series {
    let terms: Vec<(TermId, i64)> = series.terms.into_iter().map(|(coeff, power)| (coeff, power - m as i64)).collect();
    let remainder = match series.remainder {
        Remainder::ExactTruncation => Remainder::ExactTruncation,
        Remainder::BigO(_) | Remainder::LittleO(_) => {
            let pow = cc.apply_semantic(SemanticOperator::Power, vec![delta, cc.in_((order + 1) as i64)]);
            Remainder::BigO(pow)
        }
        Remainder::Unknown => Remainder::Unknown,
    };
    Series { variable: variable.to_string(), center, terms, order, remainder }
}

/// 当 `variable → +∞` 的渐近展开（经 `t = 1/x` 代换后做 Laurent，再映回 `x` 幂）。
///
/// `order`：保留的 `t` 最高幂次（即 `O(x^{-order})` 项）。结果 `center = Infinity`，项为 `coeff · x^power`。
pub fn asymptotic(cc: &mut CalculusCtx<'_>, expression: TermId, variable: &str, order: u32) -> CalculusResult<Series> {
    const T: &str = "__athena_asymp_t";
    let infinity = cc.symbol("Infinity");
    let t_sym = cc.symbol(T);
    let inv = cc.apply_semantic(SemanticOperator::Power, vec![t_sym, cc.in_(-1)]);
    let substituted = replace_symbol(cc, expression, variable, inv);
    let g = cc.eval(substituted);
    let g = clear_negative_powers_of_var(cc, g, T);
    let zero = cc.in_(0);
    match laurent(cc, g, T, zero, order) {
        CalculusResult::Exact { value: series, conditions } => {
            CalculusResult::Exact { value: remap_asymptotic_series(cc, series, variable, order), conditions }
        }
        CalculusResult::Conditional { value: series, conditions } => {
            CalculusResult::Conditional { value: remap_asymptotic_series(cc, series, variable, order), conditions }
        }
        CalculusResult::Unevaluated { .. } => CalculusResult::Unevaluated {
            expression: residual_series(cc, expression, variable, infinity, order),
            reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
        },
    }
}

/// 清除表达式中 `var` 的负幂（如 `1/(1/t+a) → t/(1+a t)`），便于在 `t=0` 展开。
fn clear_negative_powers_of_var(cc: &mut CalculusCtx<'_>, expr: TermId, var: &str) -> TermId {
    let Some((head, args)) = cc.application_head(expr)
    else {
        return expr;
    };
    match head {
        ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 => {
            if cc.number_of(args[1]).is_some_and(|n| n.is_neg_one()) {
                if let Some(k) = negative_valuation(cc, args[0], var) {
                    if k > 0 {
                        let scale = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol(var), cc.in_(k as i64)]);
                        let cleared_den = cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![args[0], scale]));
                        let den_inv = cc.apply_semantic(SemanticOperator::Power, vec![cleared_den, cc.in_(-1)]);
                        return cc.eval(cc.apply_semantic(SemanticOperator::Multiply, vec![scale, den_inv]));
                    }
                }
            }
            let base = clear_negative_powers_of_var(cc, args[0], var);
            cc.apply_semantic(SemanticOperator::Power, vec![base, args[1]])
        }
        ApplicationHead::Semantic(SemanticOperator::Add) => {
            let parts = args.iter().map(|a| clear_negative_powers_of_var(cc, *a, var)).collect();
            cc.eval(cc.apply_semantic(SemanticOperator::Add, parts))
        }
        ApplicationHead::Semantic(SemanticOperator::Multiply) => {
            let parts = args.iter().map(|a| clear_negative_powers_of_var(cc, *a, var)).collect();
            cc.eval(cc.apply_semantic(SemanticOperator::Multiply, parts))
        }
        _ => expr,
    }
}

/// `var` 在表达式中的最低整数幂次；若无负幂则 `None`。
fn negative_valuation(cc: &CalculusCtx<'_>, expr: TermId, var: &str) -> Option<u32> {
    let v = valuation(cc, expr, var)?;
    if v < 0 { Some((-v) as u32) } else { None }
}

fn valuation(cc: &CalculusCtx<'_>, expr: TermId, var: &str) -> Option<i64> {
    match cc.shape(expr)? {
        Shape::Symbol(s) if cc.symbol_is(s, var) => Some(1),
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => Some(0),
        Shape::Collection(items) => {
            let mut m = i64::MAX;
            for i in items {
                m = m.min(valuation(cc, i, var)?);
            }
            if m == i64::MAX { Some(0) } else { Some(m) }
        }
        Shape::Application(head, args) => match head {
            ApplicationHead::Semantic(SemanticOperator::Add) => {
                let mut m = i64::MAX;
                for a in args {
                    m = m.min(valuation(cc, a, var)?);
                }
                if m == i64::MAX { Some(0) } else { Some(m) }
            }
            ApplicationHead::Semantic(SemanticOperator::Multiply) => {
                let mut s = 0i64;
                for a in args {
                    s = s.saturating_add(valuation(cc, a, var)?);
                }
                Some(s)
            }
            ApplicationHead::Semantic(SemanticOperator::Power) if args.len() == 2 => {
                let base_v = valuation(cc, args[0], var)?;
                let exp = cc.int_exp(args[1])?;
                Some(base_v.saturating_mul(exp))
            }
            _ => {
                // 未知头部：若参数含 var 则保守拒绝清除
                if args.iter().any(|a| contains_symbol(cc, *a, var)) { None } else { Some(0) }
            }
        },
    }
}

fn remap_asymptotic_series(cc: &mut CalculusCtx<'_>, series: Series, variable: &str, order: u32) -> Series {
    // 无穷远处换元：g(t)=f(1/t) ~ Σ a_k tᵏ  ⇒  f(x) ~ Σ a_k x⁻ᵏ
    let terms: Vec<(TermId, i64)> = series.terms.into_iter().map(|(coeff, power)| (coeff, -power)).collect();
    let remainder = match series.remainder {
        Remainder::ExactTruncation => Remainder::ExactTruncation,
        Remainder::BigO(_) | Remainder::LittleO(_) => {
            let pow = cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol(variable), cc.in_(-(order as i64 + 1))]);
            Remainder::BigO(pow)
        }
        Remainder::Unknown => Remainder::Unknown,
    };
    let center = cc.symbol("Infinity");
    Series { variable: variable.to_string(), center, terms, order, remainder }
}

/// 系数中出现 `0^k`（k≠0）视为奇点求值失败，不得当作 Laurent 系数。
fn term_has_singular_zero_power(cc: &CalculusCtx<'_>, term: TermId) -> bool {
    match cc.shape(term) {
        Some(Shape::Collection(items)) => items.iter().any(|t| term_has_singular_zero_power(cc, *t)),
        Some(Shape::Application(ApplicationHead::Semantic(SemanticOperator::Power), args))
            if args.len() == 2 && is_zero_term(cc, args[0]) =>
        {
            !is_zero_term(cc, args[1])
        }
        Some(Shape::Application(_, args)) => args.iter().any(|t| term_has_singular_zero_power(cc, *t)),
        _ => false,
    }
}

fn is_zero_term(cc: &CalculusCtx<'_>, expr: TermId) -> bool {
    cc.number_of(expr).is_some_and(|n| n.is_zero())
}
