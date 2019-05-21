//! 特殊函数定义表 — domain / 导数 / 分支策略（arena 版 · Living `25`）。
//!
//! 微积分算法经本表查询一元函数的形式导数，而不是在 `differentiate` 里无限堆 `match` 臂。
//! 第一阶段覆盖：初等三角/双曲/反三角、`Exp`/`Log`/`Sqrt`/`Abs`/`Sign`、`Gamma`、`Erf`。

use athena_types::ExprId;

use crate::domains::calculus::ctx::CalculusCtx;

/// 分支约定 — 复数主值与实数规则不得混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BranchPolicy {
    /// 复数主值。
    Principal,
    /// 仅实数分支。
    RealOnly,
    /// 实数分段（如 `Abs`）。
    PiecewiseReal,
    /// 形式操作，不承诺解析延拓。
    Formal,
}

/// 一元函数形式导数：`f'(u)`，仍含自变量 `u`（链式法则外层再乘 `u'`）。
pub type UnaryDerivative = fn(&mut CalculusCtx<'_>, ExprId) -> ExprId;

/// 注册的函数语义。
#[derive(Debug, Clone, Copy)]
pub struct FunctionDefinition {
    /// 头部符号名（如 `Sin`、`Gamma`）。
    pub name: &'static str,
    /// 元数（引导实现仅 1）。
    pub arity: usize,
    /// 分支策略。
    pub branch: BranchPolicy,
    /// 一元形式导数；`None` 表示尚未给出（求导应保留 `D[…]`）。
    pub unary_derivative: Option<UnaryDerivative>,
}

/// 按头部名查找定义。
pub fn lookup_function(name: &str) -> Option<&'static FunctionDefinition> {
    REGISTRY.iter().find(|d| d.name == name)
}

/// 已注册函数名列表（稳定顺序）。
pub fn registered_function_names() -> impl Iterator<Item = &'static str> {
    REGISTRY.iter().map(|d| d.name)
}

static REGISTRY: &[FunctionDefinition] = &[
    FunctionDefinition { name: "Exp", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_exp) },
    FunctionDefinition { name: "Log", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_log) },
    FunctionDefinition { name: "Sin", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_sin) },
    FunctionDefinition { name: "Cos", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_cos) },
    FunctionDefinition { name: "Tan", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_tan) },
    FunctionDefinition { name: "Sinh", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_sinh) },
    FunctionDefinition { name: "Cosh", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_cosh) },
    FunctionDefinition { name: "Tanh", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_tanh) },
    FunctionDefinition { name: "ArcSin", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_arcsin) },
    FunctionDefinition { name: "ArcCos", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_arccos) },
    FunctionDefinition { name: "ArcTan", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_arctan) },
    FunctionDefinition { name: "Sqrt", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_sqrt) },
    FunctionDefinition { name: "Abs", arity: 1, branch: BranchPolicy::PiecewiseReal, unary_derivative: Some(deriv_abs) },
    FunctionDefinition { name: "Sign", arity: 1, branch: BranchPolicy::PiecewiseReal, unary_derivative: Some(deriv_sign) },
    FunctionDefinition { name: "Gamma", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_gamma) },
    FunctionDefinition { name: "Erf", arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_erf) },
];

fn deriv_exp(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    cc.ap("Exp", vec![arg])
}

fn deriv_log(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    cc.ap("Power", vec![arg, cc.in_(-1)])
}

fn deriv_sin(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    cc.ap("Cos", vec![arg])
}

fn deriv_cos(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    let sin = cc.ap("Sin", vec![arg]);
    cc.ap("Times", vec![cc.in_(-1), sin])
}

fn deriv_tan(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    let cos = cc.ap("Cos", vec![arg]);
    cc.ap("Power", vec![cos, cc.in_(-2)])
}

fn deriv_sinh(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    cc.ap("Cosh", vec![arg])
}

fn deriv_cosh(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    cc.ap("Sinh", vec![arg])
}

fn deriv_tanh(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    let cosh = cc.ap("Cosh", vec![arg]);
    cc.ap("Power", vec![cosh, cc.in_(-2)])
}

fn deriv_arcsin(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    // 形式：1/Sqrt[1-u^2]
    let u2 = cc.ap("Power", vec![arg, cc.in_(2)]);
    let one_minus = cc.ap("Plus", vec![cc.in_(1), cc.ap("Times", vec![cc.in_(-1), u2])]);
    let sqrt = cc.ap("Sqrt", vec![one_minus]);
    cc.ap("Power", vec![sqrt, cc.in_(-1)])
}

fn deriv_arccos(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    let s = deriv_arcsin(cc, arg);
    cc.ap("Times", vec![cc.in_(-1), s])
}

fn deriv_arctan(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    // 形式：1/(1+u^2)
    let u2 = cc.ap("Power", vec![arg, cc.in_(2)]);
    let one_plus = cc.ap("Plus", vec![cc.in_(1), u2]);
    cc.ap("Power", vec![one_plus, cc.in_(-1)])
}

fn deriv_sqrt(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    // 形式：1/(2 Sqrt[u])
    let sqrt = cc.ap("Sqrt", vec![arg]);
    let two_sqrt = cc.ap("Times", vec![cc.in_(2), sqrt]);
    cc.ap("Power", vec![two_sqrt, cc.in_(-1)])
}

fn deriv_abs(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    // 绝对值分支：Abs[u]/u（条件在 differentiate_checked）
    let abs = cc.ap("Abs", vec![arg]);
    let uinv = cc.ap("Power", vec![arg, cc.in_(-1)]);
    cc.ap("Times", vec![abs, uinv])
}

fn deriv_sign(_cc: &mut CalculusCtx<'_>, _arg: ExprId) -> ExprId {
    // 形式：几乎处处 0（奇点在 0 由假设处理）
    _cc.in_(0)
}

fn deriv_gamma(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    // 形式：Γ'(z) = Γ(z) PolyGamma[0, z]
    let gamma = cc.ap("Gamma", vec![arg]);
    let poly = cc.ap("PolyGamma", vec![cc.in_(0), arg]);
    cc.ap("Times", vec![gamma, poly])
}

fn deriv_erf(cc: &mut CalculusCtx<'_>, arg: ExprId) -> ExprId {
    // 形式：(2/Sqrt[Pi]) Exp[-u^2]
    let pi = cc.sym("Pi");
    let sqrt_pi = cc.ap("Sqrt", vec![pi]);
    let inv = cc.ap("Power", vec![sqrt_pi, cc.in_(-1)]);
    let u2 = cc.ap("Power", vec![arg, cc.in_(2)]);
    let neg = cc.ap("Times", vec![cc.in_(-1), u2]);
    let exp = cc.ap("Exp", vec![neg]);
    cc.ap("Times", vec![cc.in_(2), inv, exp])
}
