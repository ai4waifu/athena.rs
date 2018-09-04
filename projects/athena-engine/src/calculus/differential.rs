//! 常微分方程 — 带残差验证的一阶子集。

use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, Number};

use crate::{
    eval::evaluate,
    term::{Atom, Term, number_from_term},
};

use super::{
    derivative::differentiate,
    integral::integrate,
    result::CalculusResult,
    term_util::{contains_symbol, replace_symbol},
};

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
/// - `y' = g(x)`（无 `y`）→ `y = ∫ g`
/// - `y' = c y^n`（`n≠1`）→ 幂律特解（如 `n=2` ⇒ `-1/(c x)`）
/// - Bernoulli 常系数 `y' = a y + b y^n`（`n≠0,1`）→ 常数特解 / 退化幂律
/// - 可分离 `y' = g(x) y^n`（`n=2`）→ `y = -1/∫g`
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
    else if let Some(sol) = try_rhs_independent_of_y(&rhs.f, dependent, independent) {
        sol
    }
    else if let Some(sol) = try_power_of_y(&rhs.f, dependent, independent) {
        sol
    }
    else if let Some(sol) = try_bernoulli_const(&rhs.f, dependent, independent) {
        sol
    }
    else if let Some(sol) = try_separable_g_y_power(&rhs.f, dependent, independent) {
        sol
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
    // y' = g(x)（无 y）→ y = ∫g + C, C = y0 - F(x0)
    if !contains_symbol(f, dependent) {
        let fx0 = evaluate(&replace_symbol(particular, independent, x0));
        let c = evaluate(&Term::app("Plus", vec![y0.clone(), Term::app("Times", vec![Term::int(-1), fx0])]));
        return evaluate(&Term::app("Plus", vec![particular.clone(), c]));
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

/// `y' = g(x)`：右端不含因变量。
fn try_rhs_independent_of_y(f: &Term, dependent: &str, independent: &str) -> Option<Term> {
    if contains_symbol(f, dependent) {
        return None;
    }
    let anti = integrate(f, independent);
    if matches!(&anti, Term::Application { head, .. } if head.is_symbol("Integrate")) {
        return None;
    }
    Some(anti)
}

/// `y' = c y^n`（`n≠1`）。`n=2` ⇒ `y = -1/(c x)`。
fn try_power_of_y(f: &Term, dependent: &str, independent: &str) -> Option<Term> {
    let (c, n) = match_scaled_power_of_y(f, dependent)?;
    if n == 1 {
        return None;
    }
    if n == 2 {
        // y = -1/(c x)
        let den = evaluate(&Term::app("Times", vec![Term::number(c), Term::symbol(independent)]));
        return Some(evaluate(&Term::app("Times", vec![Term::int(-1), Term::app("Power", vec![den, Term::int(-1)])])));
    }
    // y = ((1-n) c x)^{1/(1-n)} — 仅当指数为 ±1 时构造，便于求值验证
    let one_minus_n = 1i64 - n;
    if one_minus_n == 0 {
        return None;
    }
    let inner = evaluate(&Term::app(
        "Times",
        vec![Term::integer(one_minus_n), Term::number(c), Term::symbol(independent)],
    ));
    if one_minus_n == 1 {
        Some(inner)
    } else if one_minus_n == -1 {
        Some(evaluate(&Term::app("Power", vec![inner, Term::int(-1)])))
    } else {
        None
    }
}

/// 常系数 Bernoulli：`y' = a y + b y^n`（`n≠0,1`）。
/// `a≠0` ⇒ 常数特解 `y^{n-1} = -a/b`（优先 `n=2` ⇒ `y = -a/b`）。
fn try_bernoulli_const(f: &Term, dependent: &str, independent: &str) -> Option<Term> {
    let (a, b, n) = match_bernoulli_const_rhs(f, dependent)?;
    if n == 0 || n == 1 {
        return None;
    }
    if a.is_zero() {
        // 退化为 c y^n
        return try_power_of_y(
            &evaluate(&Term::app(
                "Times",
                vec![Term::number(b), Term::app("Power", vec![Term::symbol(dependent), Term::integer(n)])],
            )),
            dependent,
            independent,
        );
    }
    if b.is_zero() {
        return None;
    }
    if n == 2 {
        // y = -a/b
        return Some(evaluate(&Term::app(
            "Times",
            vec![Term::int(-1), Term::app("Divide", vec![Term::number(a), Term::number(b)])],
        )));
    }
    None
}

/// 可分离 `y' = g(x) y^n`（bootstrap：`n=2` ⇒ `y = -1/∫g`）。
fn try_separable_g_y_power(f: &Term, dependent: &str, independent: &str) -> Option<Term> {
    let (g, n) = match_g_times_y_power(f, dependent)?;
    if n != 2 {
        return None;
    }
    if number_from_term(&g).is_some() {
        // 已由 try_power_of_y 覆盖
        return None;
    }
    if contains_symbol(&g, dependent) || !contains_symbol(&g, independent) {
        return None;
    }
    let anti = integrate(&g, independent);
    if matches!(&anti, Term::Application { head, .. } if head.is_symbol("Integrate")) {
        return None;
    }
    Some(evaluate(&Term::app("Times", vec![Term::int(-1), Term::app("Power", vec![anti, Term::int(-1)])])))
}

fn match_scaled_power_of_y(f: &Term, dependent: &str) -> Option<(Number, i64)> {
    match f {
        Term::Application { head, arguments: args }
            if head.is_symbol("Power") && args.len() == 2 && args[0].is_symbol(dependent) =>
        {
            let n = number_from_term(&args[1]).and_then(|e| e.as_integer_exp())?;
            let n_i = i64::try_from(&n).ok()?;
            Some((Number::small_int(1), n_i))
        }
        Term::Application { head, arguments: args } if head.is_symbol("Times") && args.len() == 2 => {
            if let Some(c) = number_from_term(&args[0]).cloned() {
                let (one, n) = match_scaled_power_of_y(&args[1], dependent)?;
                if !one.is_one() {
                    return None;
                }
                return Some((c, n));
            }
            if let Some(c) = number_from_term(&args[1]).cloned() {
                let (one, n) = match_scaled_power_of_y(&args[0], dependent)?;
                if !one.is_one() {
                    return None;
                }
                return Some((c, n));
            }
            None
        }
        _ => None,
    }
}

fn match_bernoulli_const_rhs(f: &Term, dependent: &str) -> Option<(Number, Number, i64)> {
    // Plus[Times[a,y], Times[b, Power[y,n]]] （两项，顺序任意）
    let Term::Application { head, arguments: args } = f
    else {
        return None;
    };
    if !head.is_symbol("Plus") || args.len() != 2 {
        return None;
    }
    let mut linear: Option<Number> = None;
    let mut power: Option<(Number, i64)> = None;
    for part in args {
        if let Some(a) = match_times_const_y(part, dependent) {
            if linear.replace(a).is_some() {
                return None;
            }
        } else if let Some((b, n)) = match_scaled_power_of_y(part, dependent) {
            if n == 1 {
                if linear.replace(b).is_some() {
                    return None;
                }
            } else if power.replace((b, n)).is_some() {
                return None;
            }
        } else {
            return None;
        }
    }
    let a = linear.unwrap_or_else(|| Number::small_int(0));
    let (b, n) = power?;
    Some((a, b, n))
}

fn match_g_times_y_power(f: &Term, dependent: &str) -> Option<(Term, i64)> {
    let Term::Application { head, arguments: args } = f
    else {
        return None;
    };
    if !head.is_symbol("Times") || args.len() != 2 {
        return None;
    }
    if let Some((one, n)) = match_scaled_power_of_y(&args[0], dependent) {
        if one.is_one() {
            return Some((args[1].clone(), n));
        }
    }
    if let Some((one, n)) = match_scaled_power_of_y(&args[1], dependent) {
        if one.is_one() {
            return Some((args[0].clone(), n));
        }
    }
    None
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
