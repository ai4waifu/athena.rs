//! 级数对象 — Taylor / Laurent / 渐近（`x→∞`）bootstrap。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    eval::evaluate,
    term::{Term, number_from_term},
};

use super::{
    derivative::differentiate,
    result::CalculusResult,
    term_util::{contains_symbol, replace_symbol},
};

/// 截断级数的余项标注。
#[derive(Debug, Clone, PartialEq)]
pub enum Remainder {
    /// 精确截断（多项式次数 ≤ order）。
    ExactTruncation,
    /// Big-O 余项（表达式）。
    BigO(Term),
    /// Little-o 余项（表达式）。
    LittleO(Term),
    /// 余项未知。
    Unknown,
}

/// 独立级数值（非裸多项式列表）。
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// 展开变量。
    pub variable: String,
    /// 展开中心（已解码；渐近于 ∞ 时为符号 `Infinity`）。
    pub center: Term,
    /// 幂次项 `(coefficient, power)`：
    /// - 有限中心：`coeff * (variable - center)^power`
    /// - `Infinity`：`coeff * variable^power`
    pub terms: Vec<(Term, i64)>,
    /// 截断阶（有限中心：最高幂次；渐近：保留的 `t=1/x` 最高幂次）。
    pub order: u32,
    /// 余项。
    pub remainder: Remainder,
}

impl Series {
    /// 展开基幂：有限中心用 `(x-c)^p`，无穷用 `x^p`。
    fn delta_power(&self, power: i64) -> Term {
        if self.center.is_symbol("Infinity") {
            if power == 0 {
                return Term::int(1);
            }
            if power == 1 {
                return Term::symbol(&self.variable);
            }
            return Term::apply("Power", vec![Term::symbol(&self.variable), Term::integer(power)]);
        }
        let delta = if is_zero_term(&self.center) {
            Term::symbol(&self.variable)
        }
        else {
            evaluate(&Term::apply(
                "Plus",
                vec![Term::symbol(&self.variable), Term::apply("Times", vec![Term::int(-1), self.center.clone()])],
            ))
        };
        if power == 0 {
            Term::int(1)
        }
        else if power == 1 {
            delta
        }
        else {
            Term::apply("Power", vec![delta, Term::integer(power)])
        }
    }

    /// 精确时转为 Plus/Times/Power 多项式项。
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
                }
                else {
                    evaluate(&Term::apply("Times", vec![coeff.clone(), self.delta_power(*power)]))
                }
            })
            .collect();
        if parts.len() == 1 { parts.into_iter().next().unwrap() } else { evaluate(&Term::apply("Plus", parts)) }
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

/// 关于 `center` 展开到 `order`（含该幂次）的 Taylor 展开。
pub fn taylor(expression: &Term, variable: &str, center: &Term, order: u32) -> CalculusResult<Series> {
    const SHIFT: &str = "__athena_taylor_t";
    let working = if is_zero_term(center) {
        expression.clone()
    }
    else {
        // f(x) 关于 c  ≡  f(t + c) 关于 t = 0。
        let shifted_var = evaluate(&Term::apply("Plus", vec![Term::symbol(SHIFT), center.clone()]));
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
                reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
            };
        }
        let coeff = if n == 0 || factorial == 1 {
            at_zero
        }
        else {
            evaluate(&Term::apply("Divide", vec![at_zero, Term::int(factorial)]))
        };
        if !is_zero_term(&coeff) {
            terms.push((coeff, n as i64));
        }
    }

    let next = evaluate(&differentiate(&current, expand_var));
    let next_at = evaluate(&replace_symbol(&next, expand_var, &Term::int(0)));
    let remainder = if is_zero_term(&next_at) && !contains_symbol(&next, expand_var) {
        Remainder::ExactTruncation
    }
    else {
        let delta = if is_zero_term(center) {
            Term::symbol(variable)
        }
        else {
            evaluate(&Term::apply(
                "Plus",
                vec![Term::symbol(variable), Term::apply("Times", vec![Term::int(-1), center.clone()])],
            ))
        };
        Remainder::BigO(Term::apply("Power", vec![delta, Term::int((order + 1) as i64)]))
    };

    CalculusResult::Exact {
        value: Series { variable: variable.to_string(), center: center.clone(), terms, order, remainder },
        conditions: Vec::new(),
    }
}

/// 关于 `center` 的 Laurent 展开：先清除有限阶极点，再 Taylor，再平移幂次。
///
/// `order` 为正则部分（非负幂）截断的最高幂次。主部在可清除时完整保留。
pub fn laurent(expression: &Term, variable: &str, center: &Term, order: u32) -> CalculusResult<Series> {
    const MAX_POLE: u32 = 8;
    let delta = if is_zero_term(center) {
        Term::symbol(variable)
    }
    else {
        evaluate(&Term::apply("Plus", vec![Term::symbol(variable), Term::apply("Times", vec![Term::int(-1), center.clone()])]))
    };

    for m in 0..=MAX_POLE {
        let cleared = if m == 0 {
            expression.clone()
        }
        else {
            evaluate(&Term::apply(
                "Times",
                vec![expression.clone(), Term::apply("Power", vec![delta.clone(), Term::int(m as i64)])],
            ))
        };
        match taylor(&cleared, variable, center, order.saturating_add(m)) {
            CalculusResult::Exact { value: series, conditions } => {
                if series.terms.iter().any(|(coeff, _)| term_has_singular_zero_power(coeff)) {
                    continue;
                }
                return CalculusResult::Exact {
                    value: remap_laurent_series(series, variable, center, order, m, &delta),
                    conditions,
                };
            }
            CalculusResult::Conditional { value: series, conditions } => {
                if series.terms.iter().any(|(coeff, _)| term_has_singular_zero_power(coeff)) {
                    continue;
                }
                return CalculusResult::Conditional {
                    value: remap_laurent_series(series, variable, center, order, m, &delta),
                    conditions,
                };
            }
            CalculusResult::Unevaluated { .. } => continue,
        }
    }

    CalculusResult::Unevaluated {
        expression: residual_series(expression, variable, center, order),
        reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
    }
}

fn remap_laurent_series(series: Series, variable: &str, center: &Term, order: u32, m: u32, delta: &Term) -> Series {
    let terms: Vec<(Term, i64)> = series.terms.into_iter().map(|(coeff, power)| (coeff, power - m as i64)).collect();
    let remainder = match series.remainder {
        Remainder::ExactTruncation => Remainder::ExactTruncation,
        Remainder::BigO(_) | Remainder::LittleO(_) => {
            Remainder::BigO(Term::apply("Power", vec![delta.clone(), Term::int((order + 1) as i64)]))
        }
        Remainder::Unknown => Remainder::Unknown,
    };
    Series { variable: variable.to_string(), center: center.clone(), terms, order, remainder }
}

/// 当 `variable → +∞` 的渐近展开（经 `t = 1/x` 代换后做 Laurent，再映回 `x` 幂）。
///
/// `order`：保留的 `t` 最高幂次（即 `O(x^{-order})` 项）。结果 `center = Infinity`，项为 `coeff · x^power`。
pub fn asymptotic(expression: &Term, variable: &str, order: u32) -> CalculusResult<Series> {
    const T: &str = "__athena_asymp_t";
    let infinity = Term::symbol("Infinity");
    let inv = Term::apply("Power", vec![Term::symbol(T), Term::int(-1)]);
    let g = evaluate(&replace_symbol(expression, variable, &inv));
    let g = clear_negative_powers_of_var(&g, T);
    match laurent(&g, T, &Term::int(0), order) {
        CalculusResult::Exact { value: series, conditions } => {
            CalculusResult::Exact { value: remap_asymptotic_series(series, variable, order), conditions }
        }
        CalculusResult::Conditional { value: series, conditions } => {
            CalculusResult::Conditional { value: remap_asymptotic_series(series, variable, order), conditions }
        }
        CalculusResult::Unevaluated { .. } => CalculusResult::Unevaluated {
            expression: residual_series(expression, variable, &infinity, order),
            reason: Diagnostic::new(DiagnosticCode::SeriesRemainderUnknown),
        },
    }
}

/// 清除表达式中 `var` 的负幂（如 `1/(1/t+a) → t/(1+a t)`），便于在 `t=0` 展开。
fn clear_negative_powers_of_var(expr: &Term, var: &str) -> Term {
    match expr {
        Term::Application { head, arguments: args } if head.is_symbol("Power") && args.len() == 2 => {
            if number_from_term(&args[1]).is_some_and(|n| n.is_neg_one()) {
                if let Some(k) = negative_valuation(&args[0], var) {
                    if k > 0 {
                        let scale = Term::apply("Power", vec![Term::symbol(var), Term::int(k as i64)]);
                        let cleared_den = evaluate(&Term::apply("Times", vec![args[0].clone(), scale.clone()]));
                        return evaluate(&Term::apply(
                            "Times",
                            vec![scale, Term::apply("Power", vec![cleared_den, Term::int(-1)])],
                        ));
                    }
                }
            }
            Term::apply("Power", vec![clear_negative_powers_of_var(&args[0], var), args[1].clone()])
        }
        Term::Application { head, arguments: args } if head.is_symbol("Plus") => {
            evaluate(&Term::apply("Plus", args.iter().map(|a| clear_negative_powers_of_var(a, var)).collect()))
        }
        Term::Application { head, arguments: args } if head.is_symbol("Times") => {
            evaluate(&Term::apply("Times", args.iter().map(|a| clear_negative_powers_of_var(a, var)).collect()))
        }
        Term::Application { head, arguments: args } => Term::Application {
            head: Box::new(clear_negative_powers_of_var(head, var)),
            arguments: args.iter().map(|a| clear_negative_powers_of_var(a, var)).collect(),
        },
        Term::List(items) => Term::List(items.iter().map(|i| clear_negative_powers_of_var(i, var)).collect()),
        Term::Atom(_) => expr.clone(),
    }
}

/// `var` 在表达式中的最低整数幂次；若无负幂则 `None`。
fn negative_valuation(expr: &Term, var: &str) -> Option<u32> {
    let v = valuation(expr, var)?;
    if v < 0 { Some((-v) as u32) } else { None }
}

fn valuation(expr: &Term, var: &str) -> Option<i64> {
    match expr {
        Term::Atom(crate::term::Atom::Symbol(s)) if s == var => Some(1),
        Term::Atom(_) => Some(0),
        Term::List(items) => {
            let mut m = i64::MAX;
            for i in items {
                m = m.min(valuation(i, var)?);
            }
            if m == i64::MAX { Some(0) } else { Some(m) }
        }
        Term::Application { head, arguments: args } => {
            let h = head.head_name().unwrap_or("");
            match h {
                "Plus" => {
                    let mut m = i64::MAX;
                    for a in args {
                        m = m.min(valuation(a, var)?);
                    }
                    if m == i64::MAX { Some(0) } else { Some(m) }
                }
                "Times" => {
                    let mut s = 0i64;
                    for a in args {
                        s = s.saturating_add(valuation(a, var)?);
                    }
                    Some(s)
                }
                "Power" if args.len() == 2 => {
                    let base_v = valuation(&args[0], var)?;
                    let exp = number_from_term(&args[1]).and_then(|e| e.as_integer_exp())?;
                    Some(base_v.saturating_mul(exp))
                }
                _ => {
                    // 未知头部：若参数含 var 则保守拒绝清除
                    if args.iter().any(|a| contains_symbol(a, var)) || contains_symbol(head, var) { None } else { Some(0) }
                }
            }
        }
    }
}

fn remap_asymptotic_series(series: Series, variable: &str, order: u32) -> Series {
    // g(t)=f(1/t) ~ Σ a_k t^k  ⇒  f(x) ~ Σ a_k x^{-k}
    let terms: Vec<(Term, i64)> = series.terms.into_iter().map(|(coeff, power)| (coeff, -power)).collect();
    let remainder = match series.remainder {
        Remainder::ExactTruncation => Remainder::ExactTruncation,
        Remainder::BigO(_) | Remainder::LittleO(_) => {
            Remainder::BigO(Term::apply("Power", vec![Term::symbol(variable), Term::integer(-(order as i64 + 1))]))
        }
        Remainder::Unknown => Remainder::Unknown,
    };
    Series { variable: variable.to_string(), center: Term::symbol("Infinity"), terms, order, remainder }
}

/// 系数中出现 `0^k`（k≠0）视为奇点求值失败，不得当作 Laurent 系数。
fn term_has_singular_zero_power(term: &Term) -> bool {
    match term {
        Term::Application { head, arguments: args } => {
            if head.is_symbol("Power") && args.len() == 2 && is_zero_term(&args[0]) {
                return !is_zero_term(&args[1]);
            }
            args.iter().any(term_has_singular_zero_power)
        }
        Term::List(items) => items.iter().any(term_has_singular_zero_power),
        Term::Atom(_) => false,
    }
}

fn is_zero_term(expr: &Term) -> bool {
    number_from_term(expr).is_some_and(|n| n.is_zero())
}
