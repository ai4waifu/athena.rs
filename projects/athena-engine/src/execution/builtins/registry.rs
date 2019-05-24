//! 特殊函数定义表 — domain / 导数 / 分支策略（arena 版 · Living `25`）。
//!
//! 微积分算法经本表查询一元函数的形式导数，而不是在 `differentiate` 里无限堆 `match` 臂。
//! 第一阶段覆盖：初等三角/双曲/反三角、`Exp`/`Log`/`Sqrt`/`Abs`/`Sign`、`Gamma`、`Erf`。

use athena_types::TermId;

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
pub type UnaryDerivative = fn(&mut CalculusCtx<'_>, TermId) -> TermId;

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

fn deriv_exp(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    cc.apply("Exp", vec![arg])
}

fn deriv_log(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    cc.apply("Power", vec![arg, cc.in_(-1)])
}

fn deriv_sin(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    cc.apply("Cos", vec![arg])
}

fn deriv_cos(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    let sin = cc.apply("Sin", vec![arg]);
    cc.apply("Times", vec![cc.in_(-1), sin])
}

fn deriv_tan(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    let cos = cc.apply("Cos", vec![arg]);
    cc.apply("Power", vec![cos, cc.in_(-2)])
}

fn deriv_sinh(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    cc.apply("Cosh", vec![arg])
}

fn deriv_cosh(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    cc.apply("Sinh", vec![arg])
}

fn deriv_tanh(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    let cosh = cc.apply("Cosh", vec![arg]);
    cc.apply("Power", vec![cosh, cc.in_(-2)])
}

fn deriv_arcsin(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    // 形式：1/Sqrt[1-u^2]
    let u2 = cc.apply("Power", vec![arg, cc.in_(2)]);
    let one_minus = cc.apply("Plus", vec![cc.in_(1), cc.apply("Times", vec![cc.in_(-1), u2])]);
    let sqrt = cc.apply("Sqrt", vec![one_minus]);
    cc.apply("Power", vec![sqrt, cc.in_(-1)])
}

fn deriv_arccos(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    let s = deriv_arcsin(cc, arg);
    cc.apply("Times", vec![cc.in_(-1), s])
}

fn deriv_arctan(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    // 形式：1/(1+u^2)
    let u2 = cc.apply("Power", vec![arg, cc.in_(2)]);
    let one_plus = cc.apply("Plus", vec![cc.in_(1), u2]);
    cc.apply("Power", vec![one_plus, cc.in_(-1)])
}

fn deriv_sqrt(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    // 形式：1/(2 Sqrt[u])
    let sqrt = cc.apply("Sqrt", vec![arg]);
    let two_sqrt = cc.apply("Times", vec![cc.in_(2), sqrt]);
    cc.apply("Power", vec![two_sqrt, cc.in_(-1)])
}

fn deriv_abs(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    // 绝对值分支：Abs[u]/u（条件在 differentiate_checked）
    let abs = cc.apply("Abs", vec![arg]);
    let uinv = cc.apply("Power", vec![arg, cc.in_(-1)]);
    cc.apply("Times", vec![abs, uinv])
}

fn deriv_sign(_cc: &mut CalculusCtx<'_>, _arg: TermId) -> TermId {
    // 形式：几乎处处 0（奇点在 0 由假设处理）
    _cc.in_(0)
}

fn deriv_gamma(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    // 形式：Γ'(z) = Γ(z) PolyGamma[0, z]
    let gamma = cc.apply("Gamma", vec![arg]);
    let poly = cc.apply("PolyGamma", vec![cc.in_(0), arg]);
    cc.apply("Times", vec![gamma, poly])
}

fn deriv_erf(cc: &mut CalculusCtx<'_>, arg: TermId) -> TermId {
    // 形式：(2/Sqrt[Pi]) Exp[-u^2]
    let pi = cc.symbol("Pi");
    let sqrt_pi = cc.apply("Sqrt", vec![pi]);
    let inv = cc.apply("Power", vec![sqrt_pi, cc.in_(-1)]);
    let u2 = cc.apply("Power", vec![arg, cc.in_(2)]);
    let neg = cc.apply("Times", vec![cc.in_(-1), u2]);
    let exp = cc.apply("Exp", vec![neg]);
    cc.apply("Times", vec![cc.in_(2), inv, exp])
}
