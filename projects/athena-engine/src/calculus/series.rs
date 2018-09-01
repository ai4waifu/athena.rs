//! 级数对象 — Taylor bootstrap（关于任意有限中心）。

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
    /// 余项未知。
    Unknown,
}

/// 独立级数值（非裸多项式列表）。
#[derive(Debug, Clone, PartialEq)]
pub struct Series {
    /// 展开变量。
    pub variable: String,
    /// 展开中心（已解码）。
    pub center: Term,
    /// 幂次项 `(coefficient, power)`，对应 `coeff * (variable - center)^power`。
    pub terms: Vec<(Term, i64)>,
    /// 截断阶（包含的最高幂次）。
    pub order: u32,
    /// 余项。
    pub remainder: Remainder,
}

impl Series {
    /// `(variable - center)` 的幂。
    fn delta_power(&self, power: i64) -> Term {
        let delta = if is_zero_term(&self.center) {
            Term::symbol(&self.variable)
        }
        else {
            evaluate(&Term::app(
                "Plus",
                vec![Term::symbol(&self.variable), Term::app("Times", vec![Term::int(-1), self.center.clone()])],
            ))
        };
        if power == 0 {
            Term::int(1)
        }
        else if power == 1 {
            delta
        }
        else {
            Term::app("Power", vec![delta, Term::integer(power)])
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
                    evaluate(&Term::app("Times", vec![coeff.clone(), self.delta_power(*power)]))
                }
            })
            .collect();
        if parts.len() == 1 { parts.into_iter().next().unwrap() } else { evaluate(&Term::app("Plus", parts)) }
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
                    "Taylor 系数仍依赖展开变量",
                ),
            };
        }
        let coeff = if n == 0 || factorial == 1 {
            at_zero
        }
        else {
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
    }
    else {
        let delta = if is_zero_term(center) {
            Term::symbol(variable)
        }
        else {
            evaluate(&Term::app("Plus", vec![Term::symbol(variable), Term::app("Times", vec![Term::int(-1), center.clone()])]))
        };
        Remainder::BigO(Term::app("Power", vec![delta, Term::int((order + 1) as i64)]))
    };

    CalculusResult::Exact {
        value: Series { variable: variable.to_string(), center: center.clone(), terms, order, remainder },
        conditions: Vec::new(),
    }
}

fn is_zero_term(expr: &Term) -> bool {
    number_from_term(expr).is_some_and(|n| n.is_zero())
}
