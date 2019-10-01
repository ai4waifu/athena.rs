//! 常微分方程 — 带残差验证的一阶子集（arena 版 · Living `25`）。

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_numeric::{Number, mul as num_mul};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, TermId};

use super::{
        derivative::differentiate,
    integral::integrate,
    result::CalculusResult,
    symbol_rewrite::{contains_symbol, replace_symbol},
};
use crate::domains::context::DomainExecutionContext;
use crate::execution::shape::Shape;

/// 候选 ODE 解是否已通过残差代入验证。
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationStatus {
    /// 残差求值为零。
    Verified {
        /// 代入后的残差表达式（应为 0）。
        residual: TermId,
    },
    /// 残差未化简为零。
    Failed {
        /// 非零残差。
        residual: TermId,
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
    pub explicit: TermId,
    /// 残差验证状态 — 发出解时必填。
    pub verified: VerificationStatus,
}

impl DifferentialSolution {
    /// 桥接项 `Equal[y[x], explicit]`。
    pub fn to_equal_term(&self, cc: &mut DomainExecutionContext<'_>) -> TermId {
        let y = cc.symbol(&self.dependent);
        let head = cc.intern_extension(&self.dependent);
        let lhs = cc.apply_extension(head, vec![y]);
        cc.apply_semantic(SemanticOperator::Equal, vec![lhs, self.explicit])
    }
}

/// 识别后的 `y' = f(x, y)` 右端。
struct FirstOrderRhs {
    /// `f`，仍可能含因变量符号。
    f: TermId,
}

/// 求解已解码方程项给出的一阶 ODE。
///
/// 引导实现支持的形态：
/// - `Equal[D[y, x], a]` → 特解 `y = a x`
/// - `Equal[D[y, x], Times[a, y]]` → 特解 `y = Exp[a x]`
/// - `Equal[Plus[D[y, x], Times[p, y]], q]`（数值 `p≠0`）→ 特解 `y = q/p`
/// - `y' = g(x)`（无 `y`）→ `y = ∫ g`
/// - `y' = c y^n`（`n≠1`）→ 幂律特解（如 `n=2` ⇒ `-1/(c x)`）
/// - Bernoulli 常系数 `y' = a y + b y^n`（`n≠0,1`）→ 常数特解 / 退化幂律
/// - 可分离 `y' = g(x) y^n`（`n=2`）→ `y = -1/∫g`
pub fn solve_ode_checked(
    cc: &mut DomainExecutionContext<'_>,
    equation: TermId,
    dependent: &str,
    independent: &str,
    initial: Option<(TermId, TermId)>,
    _assumptions: &AssumptionSet,
) -> CalculusResult<DifferentialSolution> {
    let Some(rhs) = recognize_y_prime_equals(cc, equation, dependent, independent)
    else {
        return unsupported(cc, dependent, independent, equation);
    };

    let mut explicit = if let Some(a) = cc.number_of(rhs.f).map(|n| cc.copy(n)) {
        let times = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(a), cc.symbol(independent)]);
        cc.fold_term(times)
    }
    else if let Some(a) = match_times_const_y(cc, rhs.f, dependent) {
        let times = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(a), cc.symbol(independent)]);
        let exp = cc.apply_semantic(SemanticOperator::from_unary(UnaryFunction::Exp), vec![cc.fold_term(times)]);
        exp
    }
    else if let Some((p, q)) = match_as_linear_forced(cc, rhs.f, dependent) {
        if p.is_zero() {
            return CalculusResult::Unevaluated {
                expression: placeholder(cc, dependent, independent, equation),
                reason: Diagnostic::new(DiagnosticCode::OdeUnsupported),
            };
        }
        cc.fold_term(cc.apply_semantic(SemanticOperator::Divide, vec![cc.num(q), cc.num(p)]))
    }
    else if let Some(sol) = try_rhs_independent_of_y(cc, rhs.f, dependent, independent) {
        sol
    }
    else if let Some(sol) = try_power_of_y(cc, rhs.f, dependent, independent) {
        sol
    }
    else if let Some(sol) = try_bernoulli_const(cc, rhs.f, dependent, independent) {
        sol
    }
    else if let Some(sol) = try_separable_g_y_power(cc, rhs.f, dependent, independent) {
        sol
    }
    else {
        return unsupported(cc, dependent, independent, equation);
    };

    if let Some((x0, y0)) = initial {
        explicit = apply_ivp(cc, dependent, independent, rhs.f, explicit, x0, y0);
    }

    let residual = residual_of(cc, dependent, independent, rhs.f, explicit);
    let ivp_ok = match initial {
        Some((x0, y0)) => {
            let at = cc.fold_term(replace_symbol(cc, explicit, independent, x0));
            let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), y0]);
            let sum = cc.apply_semantic(SemanticOperator::Add, vec![at, neg]);
            is_zero_term(cc, cc.fold_term(sum))
        }
        None => true,
    };

    if is_zero_term(cc, residual) && ivp_ok {
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
                verified: VerificationStatus::Failed { residual },
            },
            reason: Diagnostic::new(DiagnosticCode::OdeSolutionUnverified),
        }
    }
}

fn apply_ivp(cc: &mut DomainExecutionContext<'_>, dependent: &str, independent: &str, f: TermId, particular: TermId, x0: TermId, y0: TermId) -> TermId {
    // 常系数：y' = a → y = a·x + C，C = y0 − a·x0
    if let Some(a) = cc.number_of(f).map(|n| cc.copy(n)) {
        let ax0 = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(cc.copy(&a)), x0]));
        let c = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![y0, cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), ax0])]));
        let ax = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(a), cc.symbol(independent)]);
        return cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![ax, c]));
    }
    // 解：y' = a y → y = y0 Exp[a (x - x0)]
    if let Some(a) = match_times_const_y(cc, f, dependent) {
        let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), x0]);
        let delta = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![cc.symbol(independent), neg]));
        let exp = cc.apply_semantic(
            SemanticOperator::from_unary(UnaryFunction::Exp),
            vec![cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(a), delta])],
        );
        return cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![y0, exp]));
    }
    // 仅含自变量：y' = g(x) → y = ∫g + C，C = y0 − F(x0)
    if !contains_symbol(cc, f, dependent) {
        let fx0 = cc.fold_term(replace_symbol(cc, particular, independent, x0));
        let c = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![y0, cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), fx0])]));
        return cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![particular, c]));
    }
    // 常数特解：必要时平移
    if cc.number_of(particular).is_some() {
        return y0;
    }
    particular
}

fn residual_of(cc: &mut DomainExecutionContext<'_>, dependent: &str, independent: &str, f: TermId, explicit: TermId) -> TermId {
    let d = differentiate(cc, explicit, independent);
    let yp = cc.fold_term(d);
    let f_sub = cc.fold_term(replace_symbol(cc, f, dependent, explicit));
    let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), f_sub]);
    cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![yp, neg]))
}

/// `y' = g(x)`：右端不含因变量。
fn try_rhs_independent_of_y(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str, independent: &str) -> Option<TermId> {
    if contains_symbol(cc, f, dependent) {
        return None;
    }
    let anti = integrate(cc, f, independent);
    if is_integrate_residual(cc, anti) {
        return None;
    }
    Some(anti)
}

/// `y' = c y^n`（`n≠1`）。`n=2` ⇒ `y = -1/(c x)`。
fn try_power_of_y(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str, independent: &str) -> Option<TermId> {
    let (c, n) = match_scaled_power_of_y(cc, f, dependent)?;
    if n == 1 {
        return None;
    }
    if n == 2 {
        // 分离变量解：y = -1/(c·x)
        let den = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(c), cc.symbol(independent)]));
        let inv = cc.apply_semantic(SemanticOperator::Power, vec![den, cc.in_(-1)]);
        return Some(cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), inv])));
    }
    // 幂次分离：y = ((1−n)·c·x)^{1/(1−n)} — 仅当指数为 ±1 时构造，便于求值验证
    let one_minus_n = 1i64 - n;
    if one_minus_n == 0 {
        return None;
    }
    let inner = cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(one_minus_n), cc.num(c), cc.symbol(independent)]));
    if one_minus_n == 1 {
        Some(inner)
    }
    else if one_minus_n == -1 {
        Some(cc.fold_term(cc.apply_semantic(SemanticOperator::Power, vec![inner, cc.in_(-1)])))
    }
    else {
        None
    }
}

/// 常系数 Bernoulli：`y' = a y + b y^n`（`n≠0,1`）。
/// `a≠0` ⇒ 常数特解 `y^{n-1} = -a/b`（优先 `n=2` ⇒ `y = -a/b`）。
fn try_bernoulli_const(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str, independent: &str) -> Option<TermId> {
    let (a, b, n) = match_bernoulli_const_rhs(cc, f, dependent)?;
    if n == 0 || n == 1 {
        return None;
    }
    if a.is_zero() {
        // 退化为 c y^n
        let rhs = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(b), cc.apply_semantic(SemanticOperator::Power, vec![cc.symbol(dependent), cc.in_(n)])]);
        let rhs = cc.fold_term(rhs);
        return try_power_of_y(cc, rhs, dependent, independent);
    }
    if b.is_zero() {
        return None;
    }
    if n == 2 {
        // 平衡解：y = -a/b
        let div = cc.apply_semantic(SemanticOperator::Divide, vec![cc.num(a), cc.num(b)]);
        return Some(cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), div])));
    }
    None
}

/// 可分离 `y' = g(x) y^n`（引导实现：`n=2` ⇒ `y = -1/∫g`）。
fn try_separable_g_y_power(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str, independent: &str) -> Option<TermId> {
    let (g, n) = match_g_times_y_power(cc, f, dependent)?;
    if n != 2 {
        return None;
    }
    if cc.number_of(g).is_some() {
        // 已由 try_power_of_y 覆盖
        return None;
    }
    if contains_symbol(cc, g, dependent) || !contains_symbol(cc, g, independent) {
        return None;
    }
    let anti = integrate(cc, g, independent);
    if is_integrate_residual(cc, anti) {
        return None;
    }
    let inv = cc.apply_semantic(SemanticOperator::Power, vec![anti, cc.in_(-1)]);
    Some(cc.fold_term(cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), inv])))
}

fn match_scaled_power_of_y(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str) -> Option<(Number, i64)> {
    let Some((h, args)) = cc.application_head(f)
    else {
        return None;
    };
    if matches!(h, ApplicationHead::Semantic(SemanticOperator::Power)) && args.len() == 2 && is_symbol_named(cc, args[0], dependent) {
        let n = cc.int_exp(args[1])?;
        return Some((Number::small_int(1), n));
    }
    if matches!(h, ApplicationHead::Semantic(SemanticOperator::Multiply)) && args.len() == 2 {
        if let Some(c) = cc.number_of(args[0]).map(|n| cc.copy(n)) {
            let (one, n) = match_scaled_power_of_y(cc, args[1], dependent)?;
            if !one.is_one() {
                return None;
            }
            return Some((c, n));
        }
        if let Some(c) = cc.number_of(args[1]).map(|n| cc.copy(n)) {
            let (one, n) = match_scaled_power_of_y(cc, args[0], dependent)?;
            if !one.is_one() {
                return None;
            }
            return Some((c, n));
        }
    }
    None
}

fn match_bernoulli_const_rhs(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str) -> Option<(Number, Number, i64)> {
    // 伯努利两项：Plus[Times[a,y], Times[b, Power[y,n]]]（顺序任意）
    let (h, args) = cc.application_head(f)?;
    if !matches!(h, ApplicationHead::Semantic(SemanticOperator::Add)) || args.len() != 2 {
        return None;
    }
    let mut linear: Option<Number> = None;
    let mut power: Option<(Number, i64)> = None;
    for part in args {
        if let Some(a) = match_times_const_y(cc, part, dependent) {
            if linear.replace(a).is_some() {
                return None;
            }
        }
        else if let Some((b, n)) = match_scaled_power_of_y(cc, part, dependent) {
            if n == 1 {
                if linear.replace(b).is_some() {
                    return None;
                }
            }
            else if power.replace((b, n)).is_some() {
                return None;
            }
        }
        else {
            return None;
        }
    }
    let a = linear.unwrap_or_else(|| Number::small_int(0));
    let (b, n) = power?;
    Some((a, b, n))
}

fn match_g_times_y_power(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str) -> Option<(TermId, i64)> {
    let (h, args) = cc.application_head(f)?;
    if !matches!(h, ApplicationHead::Semantic(SemanticOperator::Multiply)) || args.len() != 2 {
        return None;
    }
    if let Some((one, n)) = match_scaled_power_of_y(cc, args[0], dependent) {
        if one.is_one() {
            return Some((args[1], n));
        }
    }
    if let Some((one, n)) = match_scaled_power_of_y(cc, args[1], dependent) {
        if one.is_one() {
            return Some((args[0], n));
        }
    }
    None
}

fn recognize_y_prime_equals(cc: &mut DomainExecutionContext<'_>, equation: TermId, dependent: &str, independent: &str) -> Option<FirstOrderRhs> {
    // 形态：Equal[D[y,x], rhs]
    let (h, args) = cc.application_head(equation)?;
    if matches!(h, ApplicationHead::Semantic(SemanticOperator::Equal)) && args.len() == 2 && is_d_of(cc, args[0], dependent, independent) {
        return Some(FirstOrderRhs { f: args[1] });
    }
    if matches!(h, ApplicationHead::Semantic(SemanticOperator::Equal)) && args.len() == 2 && is_d_of(cc, args[1], dependent, independent) {
        return Some(FirstOrderRhs { f: args[0] });
    }
    // 形态：Equal[Plus[D[y,x], Times[p,y]], q]  ⇒  y' = q - p y
    if matches!(h, ApplicationHead::Semantic(SemanticOperator::Equal)) && args.len() == 2 {
        if let Some(p) = match_d_plus_p_y(cc, args[0], dependent, independent) {
            let q = cc.number_of(args[1]).map(|n| cc.copy(n)).unwrap_or_else(|| Number::small_int(0));
            let py = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.num(cc.copy(&p)), cc.symbol(dependent)]);
            let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), py]);
            let f = cc.fold_term(cc.apply_semantic(SemanticOperator::Add, vec![cc.num(q), neg]));
            return Some(FirstOrderRhs { f });
        }
    }
    None
}

fn match_d_plus_p_y(cc: &mut DomainExecutionContext<'_>, term: TermId, dependent: &str, independent: &str) -> Option<Number> {
    let (h, args) = cc.application_head(term)?;
    if !matches!(h, ApplicationHead::Semantic(SemanticOperator::Add)) || args.len() != 2 {
        return None;
    }
    if is_d_of(cc, args[0], dependent, independent) {
        return match_times_const_y(cc, args[1], dependent);
    }
    if is_d_of(cc, args[1], dependent, independent) {
        return match_times_const_y(cc, args[0], dependent);
    }
    None
}

fn match_as_linear_forced(cc: &mut DomainExecutionContext<'_>, f: TermId, dependent: &str) -> Option<(Number, Number)> {
    // 形态：f = q + Times[-1, p, y] 或 Plus[q, Times[-p, y]]
    let (h, args) = cc.application_head(f)?;
    if !matches!(h, ApplicationHead::Semantic(SemanticOperator::Add)) || args.len() != 2 {
        return None;
    }
    let (q_term, py_term) = if cc.number_of(args[0]).is_some() {
        (args[0], args[1])
    }
    else if cc.number_of(args[1]).is_some() {
        (args[1], args[0])
    }
    else {
        return None;
    };
    let q = cc.copy(cc.number_of(q_term)?);
    let (th, targs) = cc.application_head(py_term)?;
    if !matches!(th, ApplicationHead::Semantic(SemanticOperator::Multiply)) {
        return None;
    }
    let mut coef = Number::small_int(1);
    let mut saw_y = false;
    for t in targs {
        if is_symbol_named(cc, t, dependent) {
            saw_y = true;
        }
        else if let Some(n) = cc.number_of(t) {
            coef = num_mul(coef, cc.copy(n)).ok()?;
        }
        else {
            return None;
        }
    }
    if !saw_y {
        return None;
    }
    let p = num_mul(coef, Number::small_int(-1)).ok()?;
    Some((p, q))
}

fn is_d_of(cc: &DomainExecutionContext<'_>, term: TermId, dependent: &str, independent: &str) -> bool {
    let Some((h, args)) = cc.application_head(term)
    else {
        return false;
    };
    matches!(h, ApplicationHead::Semantic(SemanticOperator::Differentiate))
        && args.len() == 2
        && is_symbol_named(cc, args[0], dependent)
        && is_symbol_named(cc, args[1], independent)
}

fn is_integrate_residual(cc: &DomainExecutionContext<'_>, term: TermId) -> bool {
    matches!(
        cc.application_head(term),
        Some((ApplicationHead::Semantic(SemanticOperator::Integrate), _))
    )
}

fn is_symbol_named(cc: &DomainExecutionContext<'_>, term: TermId, name: &str) -> bool {
    matches!(cc.shape(term), Some(Shape::Symbol(s)) if cc.symbol_id_is(s, cc.intern(name)))
}

fn match_times_const_y(cc: &mut DomainExecutionContext<'_>, term: TermId, dependent: &str) -> Option<Number> {
    let Some((h, args)) = cc.application_head(term)
    else {
        return None;
    };
    if matches!(h, ApplicationHead::Semantic(SemanticOperator::Multiply)) && args.len() == 2 {
        if is_symbol_named(cc, args[1], dependent) {
            return cc.number_of(args[0]).map(|n| cc.copy(n));
        }
        if is_symbol_named(cc, args[0], dependent) {
            return cc.number_of(args[1]).map(|n| cc.copy(n));
        }
        return None;
    }
    None
}

fn is_zero_term(cc: &DomainExecutionContext<'_>, expr: TermId) -> bool {
    cc.number_of(expr).is_some_and(|n| n.is_zero())
}

fn placeholder(cc: &mut DomainExecutionContext<'_>, dependent: &str, independent: &str, equation: TermId) -> DifferentialSolution {
    DifferentialSolution {
        dependent: dependent.to_string(),
        independent: independent.to_string(),
        explicit: equation,
        verified: VerificationStatus::Failed { residual: cc.symbol("Unevaluated") },
    }
}

fn unsupported(cc: &mut DomainExecutionContext<'_>, dependent: &str, independent: &str, equation: TermId) -> CalculusResult<DifferentialSolution> {
    CalculusResult::Unevaluated {
        expression: placeholder(cc, dependent, independent, equation),
        reason: Diagnostic::new(DiagnosticCode::OdeUnsupported),
    }
}
