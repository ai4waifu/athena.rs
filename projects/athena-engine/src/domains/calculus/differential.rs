//! 常微分方程 — 带残差验证的一阶子集（arena 版 · Living `25`）。

use athena_numeric::{Number, mul as num_mul};
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, ExprId};

use super::{
    ctx::CalculusCtx,
    derivative::differentiate,
    expression_util::{contains_symbol, replace_symbol},
    integral::integrate,
    result::CalculusResult,
};
use crate::execution::vm::Shape;

/// 候选 ODE 解是否已通过残差代入验证。
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationStatus {
    /// 残差求值为零。
    Verified {
        /// 代入后的残差表达式（应为 0）。
        residual: ExprId,
    },
    /// 残差未化简为零。
    Failed {
        /// 非零残差。
        residual: ExprId,
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
    pub explicit: ExprId,
    /// 残差验证状态 — 发出解时必填。
    pub verified: VerificationStatus,
}

impl DifferentialSolution {
    /// 桥接项 `Equal[y[x], explicit]`。
    pub fn to_equal_term(&self, cc: &mut CalculusCtx<'_>) -> ExprId {
        let y = cc.sym(&self.dependent);
        let lhs = cc.ap(&self.dependent, vec![y]);
        cc.ap("Equal", vec![lhs, self.explicit])
    }
}

/// 识别后的 `y' = f(x, y)` 右端。
struct FirstOrderRhs {
    /// `f`，仍可能含因变量符号。
    f: ExprId,
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
    cc: &mut CalculusCtx<'_>,
    equation: ExprId,
    dependent: &str,
    independent: &str,
    initial: Option<(ExprId, ExprId)>,
    _assumptions: &AssumptionSet,
) -> CalculusResult<DifferentialSolution> {
    let Some(rhs) = recognize_y_prime_equals(cc, equation, dependent, independent)
    else {
        return unsupported(cc, dependent, independent, equation);
    };

    let mut explicit = if let Some(a) = cc.number_of(rhs.f).map(|n| cc.copy(n)) {
        let times = cc.ap("Times", vec![cc.num(a), cc.sym(independent)]);
        cc.eval(times)
    }
    else if let Some(a) = match_times_const_y(cc, rhs.f, dependent) {
        let times = cc.ap("Times", vec![cc.num(a), cc.sym(independent)]);
        let exp = cc.ap("Exp", vec![cc.eval(times)]);
        exp
    }
    else if let Some((p, q)) = match_as_linear_forced(cc, rhs.f, dependent) {
        if p.is_zero() {
            return CalculusResult::Unevaluated {
                expression: placeholder(cc, dependent, independent, equation),
                reason: Diagnostic::new(DiagnosticCode::OdeUnsupported),
            };
        }
        cc.eval(cc.ap("Divide", vec![cc.num(q), cc.num(p)]))
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
            let at = cc.eval(replace_symbol(cc, explicit, independent, x0));
            let neg = cc.ap("Times", vec![cc.in_(-1), y0]);
            let sum = cc.ap("Plus", vec![at, neg]);
            is_zero_term(cc, cc.eval(sum))
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

fn apply_ivp(
    cc: &mut CalculusCtx<'_>,
    dependent: &str,
    independent: &str,
    f: ExprId,
    particular: ExprId,
    x0: ExprId,
    y0: ExprId,
) -> ExprId {
    // 常系数：y' = a → y = a·x + C，C = y0 − a·x0
    if let Some(a) = cc.number_of(f).map(|n| cc.copy(n)) {
        let ax0 = cc.eval(cc.ap("Times", vec![cc.num(cc.copy(&a)), x0]));
        let c = cc.eval(cc.ap("Plus", vec![y0, cc.ap("Times", vec![cc.in_(-1), ax0])]));
        let ax = cc.ap("Times", vec![cc.num(a), cc.sym(independent)]);
        return cc.eval(cc.ap("Plus", vec![ax, c]));
    }
    // 解：y' = a y → y = y0 Exp[a (x - x0)]
    if let Some(a) = match_times_const_y(cc, f, dependent) {
        let neg = cc.ap("Times", vec![cc.in_(-1), x0]);
        let delta = cc.eval(cc.ap("Plus", vec![cc.sym(independent), neg]));
        let exp = cc.ap("Exp", vec![cc.ap("Times", vec![cc.num(a), delta])]);
        return cc.eval(cc.ap("Times", vec![y0, exp]));
    }
    // 仅含自变量：y' = g(x) → y = ∫g + C，C = y0 − F(x0)
    if !contains_symbol(cc, f, dependent) {
        let fx0 = cc.eval(replace_symbol(cc, particular, independent, x0));
        let c = cc.eval(cc.ap("Plus", vec![y0, cc.ap("Times", vec![cc.in_(-1), fx0])]));
        return cc.eval(cc.ap("Plus", vec![particular, c]));
    }
    // 常数特解：必要时平移
    if cc.number_of(particular).is_some() {
        return y0;
    }
    particular
}

fn residual_of(cc: &mut CalculusCtx<'_>, dependent: &str, independent: &str, f: ExprId, explicit: ExprId) -> ExprId {
    let d = differentiate(cc, explicit, independent);
    let yp = cc.eval(d);
    let f_sub = cc.eval(replace_symbol(cc, f, dependent, explicit));
    let neg = cc.ap("Times", vec![cc.in_(-1), f_sub]);
    cc.eval(cc.ap("Plus", vec![yp, neg]))
}

/// `y' = g(x)`：右端不含因变量。
fn try_rhs_independent_of_y(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str, independent: &str) -> Option<ExprId> {
    if contains_symbol(cc, f, dependent) {
        return None;
    }
    let anti = integrate(cc, f, independent);
    if cc.head_name(anti).is_some_and(|h| h == "Integrate") {
        return None;
    }
    Some(anti)
}

/// `y' = c y^n`（`n≠1`）。`n=2` ⇒ `y = -1/(c x)`。
fn try_power_of_y(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str, independent: &str) -> Option<ExprId> {
    let (c, n) = match_scaled_power_of_y(cc, f, dependent)?;
    if n == 1 {
        return None;
    }
    if n == 2 {
        // 分离变量解：y = -1/(c·x)
        let den = cc.eval(cc.ap("Times", vec![cc.num(c), cc.sym(independent)]));
        let inv = cc.ap("Power", vec![den, cc.in_(-1)]);
        return Some(cc.eval(cc.ap("Times", vec![cc.in_(-1), inv])));
    }
    // 幂次分离：y = ((1−n)·c·x)^{1/(1−n)} — 仅当指数为 ±1 时构造，便于求值验证
    let one_minus_n = 1i64 - n;
    if one_minus_n == 0 {
        return None;
    }
    let inner = cc.eval(cc.ap("Times", vec![cc.in_(one_minus_n), cc.num(c), cc.sym(independent)]));
    if one_minus_n == 1 {
        Some(inner)
    }
    else if one_minus_n == -1 {
        Some(cc.eval(cc.ap("Power", vec![inner, cc.in_(-1)])))
    }
    else {
        None
    }
}

/// 常系数 Bernoulli：`y' = a y + b y^n`（`n≠0,1`）。
/// `a≠0` ⇒ 常数特解 `y^{n-1} = -a/b`（优先 `n=2` ⇒ `y = -a/b`）。
fn try_bernoulli_const(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str, independent: &str) -> Option<ExprId> {
    let (a, b, n) = match_bernoulli_const_rhs(cc, f, dependent)?;
    if n == 0 || n == 1 {
        return None;
    }
    if a.is_zero() {
        // 退化为 c y^n
        let rhs = cc.ap("Times", vec![cc.num(b), cc.ap("Power", vec![cc.sym(dependent), cc.in_(n)])]);
        let rhs = cc.eval(rhs);
        return try_power_of_y(cc, rhs, dependent, independent);
    }
    if b.is_zero() {
        return None;
    }
    if n == 2 {
        // 平衡解：y = -a/b
        let div = cc.ap("Divide", vec![cc.num(a), cc.num(b)]);
        return Some(cc.eval(cc.ap("Times", vec![cc.in_(-1), div])));
    }
    None
}

/// 可分离 `y' = g(x) y^n`（引导实现：`n=2` ⇒ `y = -1/∫g`）。
fn try_separable_g_y_power(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str, independent: &str) -> Option<ExprId> {
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
    if cc.head_name(anti).is_some_and(|h| h == "Integrate") {
        return None;
    }
    let inv = cc.ap("Power", vec![anti, cc.in_(-1)]);
    Some(cc.eval(cc.ap("Times", vec![cc.in_(-1), inv])))
}

fn match_scaled_power_of_y(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str) -> Option<(Number, i64)> {
    let Some((h, args)) = cc.app(f)
    else {
        return None;
    };
    if h == "Power" && args.len() == 2 && is_sym_named(cc, args[0], dependent) {
        let n = cc.int_exp(args[1])?;
        return Some((Number::small_int(1), n));
    }
    if h == "Times" && args.len() == 2 {
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

fn match_bernoulli_const_rhs(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str) -> Option<(Number, Number, i64)> {
    // 伯努利两项：Plus[Times[a,y], Times[b, Power[y,n]]]（顺序任意）
    let (h, args) = cc.app(f)?;
    if h != "Plus" || args.len() != 2 {
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

fn match_g_times_y_power(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str) -> Option<(ExprId, i64)> {
    let (h, args) = cc.app(f)?;
    if h != "Times" || args.len() != 2 {
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

fn recognize_y_prime_equals(
    cc: &mut CalculusCtx<'_>,
    equation: ExprId,
    dependent: &str,
    independent: &str,
) -> Option<FirstOrderRhs> {
    // 形态：Equal[D[y,x], rhs]
    let (h, args) = cc.app(equation)?;
    if h == "Equal" && args.len() == 2 && is_d_of(cc, args[0], dependent, independent) {
        return Some(FirstOrderRhs { f: args[1] });
    }
    if h == "Equal" && args.len() == 2 && is_d_of(cc, args[1], dependent, independent) {
        return Some(FirstOrderRhs { f: args[0] });
    }
    // 形态：Equal[Plus[D[y,x], Times[p,y]], q]  ⇒  y' = q - p y
    if h == "Equal" && args.len() == 2 {
        if let Some(p) = match_d_plus_p_y(cc, args[0], dependent, independent) {
            let q = cc.number_of(args[1]).map(|n| cc.copy(n)).unwrap_or_else(|| Number::small_int(0));
            let py = cc.ap("Times", vec![cc.num(cc.copy(&p)), cc.sym(dependent)]);
            let neg = cc.ap("Times", vec![cc.in_(-1), py]);
            let f = cc.eval(cc.ap("Plus", vec![cc.num(q), neg]));
            return Some(FirstOrderRhs { f });
        }
    }
    None
}

fn match_d_plus_p_y(cc: &mut CalculusCtx<'_>, term: ExprId, dependent: &str, independent: &str) -> Option<Number> {
    let (h, args) = cc.app(term)?;
    if h != "Plus" || args.len() != 2 {
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

fn match_as_linear_forced(cc: &mut CalculusCtx<'_>, f: ExprId, dependent: &str) -> Option<(Number, Number)> {
    // 形态：f = q + Times[-1, p, y] 或 Plus[q, Times[-p, y]]
    let (h, args) = cc.app(f)?;
    if h != "Plus" || args.len() != 2 {
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
    let (th, targs) = cc.app(py_term)?;
    if th != "Times" {
        return None;
    }
    let mut coef = Number::small_int(1);
    let mut saw_y = false;
    for t in targs {
        if is_sym_named(cc, t, dependent) {
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

fn is_d_of(cc: &CalculusCtx<'_>, term: ExprId, dependent: &str, independent: &str) -> bool {
    let Some((h, args)) = cc.app(term)
    else {
        return false;
    };
    h == "D" && args.len() == 2 && is_sym_named(cc, args[0], dependent) && is_sym_named(cc, args[1], independent)
}

fn is_sym_named(cc: &CalculusCtx<'_>, term: ExprId, name: &str) -> bool {
    matches!(cc.shape(term), Some(Shape::Sym(s)) if cc.sym_is(s, name))
}

fn match_times_const_y(cc: &mut CalculusCtx<'_>, term: ExprId, dependent: &str) -> Option<Number> {
    let Some((h, args)) = cc.app(term)
    else {
        return None;
    };
    if h == "Times" && args.len() == 2 {
        if is_sym_named(cc, args[1], dependent) {
            return cc.number_of(args[0]).map(|n| cc.copy(n));
        }
        if is_sym_named(cc, args[0], dependent) {
            return cc.number_of(args[1]).map(|n| cc.copy(n));
        }
        return None;
    }
    None
}

fn is_zero_term(cc: &CalculusCtx<'_>, expr: ExprId) -> bool {
    cc.number_of(expr).is_some_and(|n| n.is_zero())
}

fn placeholder(cc: &mut CalculusCtx<'_>, dependent: &str, independent: &str, equation: ExprId) -> DifferentialSolution {
    DifferentialSolution {
        dependent: dependent.to_string(),
        independent: independent.to_string(),
        explicit: equation,
        verified: VerificationStatus::Failed { residual: cc.sym("Unevaluated") },
    }
}

fn unsupported(
    cc: &mut CalculusCtx<'_>,
    dependent: &str,
    independent: &str,
    equation: ExprId,
) -> CalculusResult<DifferentialSolution> {
    CalculusResult::Unevaluated {
        expression: placeholder(cc, dependent, independent, equation),
        reason: Diagnostic::new(DiagnosticCode::OdeUnsupported),
    }
}
