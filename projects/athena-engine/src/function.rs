//! 特殊函数定义表 — domain / 导数 / 分支策略（引导实现）。
//!
//! 微积分算法经本表查询一元函数的形式导数，而不是在 `differentiate` 里无限堆 `match` 臂。
//! 第一阶段覆盖：初等三角/双曲/反三角、`Exp`/`Log`/`Sqrt`/`Abs`/`Sign`、`Gamma`、`Erf`。

use crate::numeric_clone::{clone_term};
use crate::term::Term;

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
pub type UnaryDerivative = fn(arg: &Term) -> Term;

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

fn deriv_exp(arg: &Term) -> Term {
    Term::apply("Exp", vec![clone_term(arg)])
}

fn deriv_log(arg: &Term) -> Term {
    Term::apply("Power", vec![clone_term(arg), Term::int(-1)])
}

fn deriv_sin(arg: &Term) -> Term {
    Term::apply("Cos", vec![clone_term(arg)])
}

fn deriv_cos(arg: &Term) -> Term {
    Term::apply("Times", vec![Term::int(-1), Term::apply("Sin", vec![clone_term(arg)])])
}

fn deriv_tan(arg: &Term) -> Term {
    Term::apply("Power", vec![Term::apply("Cos", vec![clone_term(arg)]), Term::int(-2)])
}

fn deriv_sinh(arg: &Term) -> Term {
    Term::apply("Cosh", vec![clone_term(arg)])
}

fn deriv_cosh(arg: &Term) -> Term {
    Term::apply("Sinh", vec![clone_term(arg)])
}

fn deriv_tanh(arg: &Term) -> Term {
    Term::apply("Power", vec![Term::apply("Cosh", vec![clone_term(arg)]), Term::int(-2)])
}

fn deriv_arcsin(arg: &Term) -> Term {
    // 形式：1/Sqrt[1-u^2]
    Term::apply(
        "Power",
        vec![
            Term::apply(
                "Sqrt",
                vec![Term::apply(
                    "Plus",
                    vec![
                        Term::int(1),
                        Term::apply("Times", vec![Term::int(-1), Term::apply("Power", vec![clone_term(arg), Term::int(2)])]),
                    ],
                )],
            ),
            Term::int(-1),
        ],
    )
}

fn deriv_arccos(arg: &Term) -> Term {
    Term::apply("Times", vec![Term::int(-1), deriv_arcsin(arg)])
}

fn deriv_arctan(arg: &Term) -> Term {
    // 形式：1/(1+u^2)
    Term::apply(
        "Power",
        vec![Term::apply("Plus", vec![Term::int(1), Term::apply("Power", vec![clone_term(arg), Term::int(2)])]), Term::int(-1)],
    )
}

fn deriv_sqrt(arg: &Term) -> Term {
    // 形式：1/(2 Sqrt[u])
    Term::apply("Power", vec![Term::apply("Times", vec![Term::int(2), Term::apply("Sqrt", vec![clone_term(arg)])]), Term::int(-1)])
}

fn deriv_abs(arg: &Term) -> Term {
    // 绝对值分支：Abs[u]/u（条件在 differentiate_checked）
    Term::apply("Times", vec![Term::apply("Abs", vec![clone_term(arg)]), Term::apply("Power", vec![clone_term(arg), Term::int(-1)])])
}

fn deriv_sign(_arg: &Term) -> Term {
    // 形式：几乎处处 0（奇点在 0 由假设处理）
    Term::int(0)
}

fn deriv_gamma(arg: &Term) -> Term {
    // 形式：Γ'(z) = Γ(z) PolyGamma[0, z]
    Term::apply(
        "Times",
        vec![Term::apply("Gamma", vec![clone_term(arg)]), Term::apply("PolyGamma", vec![Term::int(0), clone_term(arg)])],
    )
}

fn deriv_erf(arg: &Term) -> Term {
    // 形式：(2/Sqrt[Pi]) Exp[-u^2]
    Term::apply(
        "Times",
        vec![
            Term::int(2),
            Term::apply("Power", vec![Term::apply("Sqrt", vec![Term::symbol("Pi")]), Term::int(-1)]),
            Term::apply(
                "Exp",
                vec![Term::apply("Times", vec![Term::int(-1), Term::apply("Power", vec![clone_term(arg), Term::int(2)])])],
            ),
        ],
    )
}
