//! 常微分方程 — 带残差验证的一阶子集。

use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, Number};

use crate::{
    eval::evaluate,
    term::{Atom, Term, number_from_term},
};

use super::{derivative::differentiate, result::CalculusResult, term_util::replace_symbol};

/// 候选 ODE 解是否已通过残差代入验证。
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationStatus {
    /// 残差求值为零。
    Verified {
        /// 代入后的残差表达式（应为 0）。
        residual: Term,
    },
    /// 残差未化简为零。
    Failed {
        /// 非零残差。
        residual: Term,
    },
}

/// 显式一阶 ODE 解对象（非裸项）。
#[derive(Debug, Clone, PartialEq)]
pub struct DifferentialSolution {
    /// 因变量名（桥接）。
    pub dependent: String,
    /// 自变量名。
    pub independent: String,
    /// `y(x)` 的显式特解右端。
    pub explicit: Term,
    /// 残差验证状态 — 发出解时必填。
    pub verified: VerificationStatus,
}

impl DifferentialSolution {
    /// 桥接项 `Equal[y[x], explicit]`。
    pub fn to_equal_term(&self) -> Term {
        Term::app(
            "Equal",
            vec![Term::app(self.dependent.as_str(), vec![Term::symbol(&self.independent)]), self.explicit.clone()],
        )
    }
}

/// 识别后的 `y' = f(x, y)` 右端。
struct FirstOrderRhs {
    /// `f`，仍可能含因变量符号。
    f: Term,
}

/// 求解已解码方程项给出的一阶 ODE。
///
/// Bootstrap 形态：
/// - `Equal[D[y, x], a]` → 特解 `y = a x`
/// - `Equal[D[y, x], Times[a, y]]` → 特解 `y = Exp[a x]`
/// - `Equal[Plus[D[y, x], Times[p, y]], q]`（数值 `p≠0`）→ 特解 `y = q/p`
pub fn solve_ode_checked(
    equation: &Term,
    dependent: &str,
    independent: &str,
    initial: Option<&(Term, Term)>,
    _assumptions: &AssumptionSet,
) -> CalculusResult<DifferentialSolution> {
    let Some(rhs) = recognize_y_prime_equals(equation, dependent, independent)
    else {
        return unsupported(dependent, independent, equation);
    };

    let mut explicit = if let Some(a) = number_from_term(&rhs.f).cloned() {
        evaluate(&Term::app("Times", vec![Term::number(a), Term::symbol(independent)]))
    }
    else if let Some(a) = match_times_const_y(&rhs.f, dependent) {
        Term::app("Exp", vec![evaluate(&Term::app("Times", vec![Term::number(a), Term::symbol(independent)]))])
    }
    else if let Some((p, q)) = match_as_linear_forced(&rhs.f, dependent) {
        if p.is_zero() {
            return CalculusResult::Unevaluated {
                expression: placeholder(dependent, independent, equation.clone()),
                reason: Diagnostic::error(DiagnosticCode::OdeUnsupported, "线性 ODE 阻尼系数为 0"),
            };
        }
        evaluate(&Term::app("Divide", vec![Term::number(q), Term::number(p)]))
    }
    else {
        return unsupported(dependent, independent, equation);
    };

    if let Some((x0, y0)) = initial {
        explicit = apply_ivp(dependent, independent, &rhs.f, &explicit, x0, y0);
    }

    let residual = residual_of(dependent, independent, &rhs.f, &explicit);
    let ivp_ok = match initial {
        Some((x0, y0)) => {
            let at = evaluate(&replace_symbol(&explicit, independent, x0));
            is_zero_term(&evaluate(&Term::app("Plus", vec![at, Term::app("Times", vec![Term::int(-1), y0.clone()])])))
        }
        None => true,
    };

    if is_zero_term(&residual) && ivp_ok {
        CalculusResult::Exact {
            value: DifferentialSolution {
                dependent: dependent.to_string(),
                independent: independent.to_string(),
                explicit,
                verified: VerificationStatus::Verified { residual },
            },
            conditions: Vec::new(),
        }
    }
    else {
        CalculusResult::Unevaluated {
            expression: DifferentialSolution {
                dependent: dependent.to_string(),
                independent: independent.to_string(),
                explicit,
                verified: VerificationStatus::Failed { residual: residual.clone() },
            },
            reason: Diagnostic::error(
                DiagnosticCode::OdeSolutionUnverified,
                format!("ODE 残差/初值未归零: residual={residual:?}"),
            ),
        }
    }
}

fn apply_ivp(dependent: &str, independent: &str, f: &Term, particular: &Term, x0: &Term, y0: &Term) -> Term {
    // y' = a (常数) → y = a x + C, C = y0 - a x0
    if let Some(a) = number_from_term(f).cloned() {
        let ax0 = evaluate(&Term::app("Times", vec![Term::number(a.clone()), x0.clone()]));
        let c = evaluate(&Term::app("Plus", vec![y0.clone(), Term::app("Times", vec![Term::int(-1), ax0])]));
        return evaluate(&Term::app("Plus", vec![Term::app("Times", vec![Term::number(a), Term::symbol(independent)]), c]));
    }
    // y' = a y → y = y0 Exp[a (x - x0)]
    if let Some(a) = match_times_const_y(f, dependent) {
        let delta =
            evaluate(&Term::app("Plus", vec![Term::symbol(independent), Term::app("Times", vec![Term::int(-1), x0.clone()])]));
        return evaluate(&Term::app(
            "Times",
            vec![y0.clone(), Term::app("Exp", vec![Term::app("Times", vec![Term::number(a), delta])])],
        ));
    }
    // 常数特解：必要时平移
    if number_from_term(particular).is_some() {
        return y0.clone();
    }
    particular.clone()
}

fn residual_of(dependent: &str, independent: &str, f: &Term, explicit: &Term) -> Term {
    let yp = evaluate(&differentiate(explicit, independent));
    let f_sub = evaluate(&replace_symbol(f, dependent, explicit));
    evaluate(&Term::app("Plus", vec![yp, Term::app("Times", vec![Term::int(-1), f_sub])]))
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
                    vec![Term::number(q), Term::app("Times", vec![Term::int(-1), Term::number(p), Term::symbol(dependent)])],
                ));
                return Some(FirstOrderRhs { f });
            }
        }
    }
    None
}

fn match_d_plus_p_y(term: &Term, dependent: &str, independent: &str) -> Option<Number> {
    let Term::Application { head, arguments: args } = term
    else {
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
    // f = q + Times[-1, p, y] 或 Plus[q, Times[-p, y]]
    match f {
        Term::Application { head, arguments: args } if head.is_symbol("Plus") && args.len() == 2 => {
            let (q_term, py_term) = if number_from_term(&args[0]).is_some() {
                (&args[0], &args[1])
            }
            else if number_from_term(&args[1]).is_some() {
                (&args[1], &args[0])
            }
            else {
                return None;
            };
            let q = number_from_term(q_term)?.clone();
            let Term::Application { head: th, arguments: targs } = py_term
            else {
                return None;
            };
            if !th.is_symbol("Times") {
                return None;
            }
            // Times[-1, p, y] 或 Times[-p, y]
            let mut coef = Number::small_int(1);
            let mut saw_y = false;
            for t in targs {
                if t.is_symbol(dependent) {
                    saw_y = true;
                }
                else if let Some(n) = number_from_term(t) {
                    coef = coef.mul(n.clone()).ok()?;
                }
                else {
                    return None;
                }
            }
            if !saw_y {
                return None;
            }
            // f = q + coef*y 且 coef = -p ⇒ p = -coef
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
        verified: VerificationStatus::Failed { residual: Term::symbol("Unevaluated") },
    }
}

fn unsupported(dependent: &str, independent: &str, equation: &Term) -> CalculusResult<DifferentialSolution> {
    CalculusResult::Unevaluated {
        expression: placeholder(dependent, independent, equation.clone()),
        reason: Diagnostic::error(DiagnosticCode::OdeUnsupported, "ODE 类型不在一阶 bootstrap 子集内"),
    }
}
