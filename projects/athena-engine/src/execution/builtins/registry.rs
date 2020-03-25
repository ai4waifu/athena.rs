//! 特殊函数定义表 — domain / 导数 / 分支策略（arena 版 · ）。
//!
//! 按封闭 [`UnaryFunction`] 身份查询，禁止字符串名分派。

use athena_ir::{SemanticOperator, UnaryFunction};
use athena_types::TermId;

use crate::domains::context::DomainExecutionContext;

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
pub type UnaryDerivative = fn(&mut DomainExecutionContext<'_>, TermId) -> TermId;

/// 注册的函数语义。
#[derive(Debug, Clone, Copy)]
pub struct FunctionDefinition {
    /// 封闭一元函数身份。
    pub function: UnaryFunction,
    /// 元数（引导实现仅 1）。
    pub arity: usize,
    /// 分支策略。
    pub branch: BranchPolicy,
    /// 一元形式导数；`None` 表示尚未给出（求导应保留残差）。
    pub unary_derivative: Option<UnaryDerivative>,
}

impl FunctionDefinition {
    /// 构造该函数应用时优先使用的语义头。
    pub const fn operator(self) -> SemanticOperator {
        SemanticOperator::from_unary(self.function)
    }
}

/// 按封闭一元函数身份查找定义。
pub fn lookup_unary(function: UnaryFunction) -> Option<&'static FunctionDefinition> {
    REGISTRY.iter().find(|d| d.function == function)
}

/// 已注册一元函数列表（稳定顺序）。
pub fn registered_unary_functions() -> impl Iterator<Item = UnaryFunction> {
    REGISTRY.iter().map(|d| d.function)
}

static REGISTRY: &[FunctionDefinition] = &[
    FunctionDefinition { function: UnaryFunction::Exp, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_exp) },
    FunctionDefinition { function: UnaryFunction::Log, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_log) },
    FunctionDefinition { function: UnaryFunction::Sin, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_sin) },
    FunctionDefinition { function: UnaryFunction::Cos, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_cos) },
    FunctionDefinition { function: UnaryFunction::Tan, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_tan) },
    FunctionDefinition { function: UnaryFunction::Sinh, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_sinh) },
    FunctionDefinition { function: UnaryFunction::Cosh, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_cosh) },
    FunctionDefinition { function: UnaryFunction::Tanh, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_tanh) },
    FunctionDefinition { function: UnaryFunction::ArcSin, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_arcsin) },
    FunctionDefinition { function: UnaryFunction::ArcCos, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_arccos) },
    FunctionDefinition { function: UnaryFunction::ArcTan, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_arctan) },
    FunctionDefinition { function: UnaryFunction::Sqrt, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_sqrt) },
    FunctionDefinition { function: UnaryFunction::Abs, arity: 1, branch: BranchPolicy::PiecewiseReal, unary_derivative: Some(deriv_abs) },
    FunctionDefinition { function: UnaryFunction::Sign, arity: 1, branch: BranchPolicy::PiecewiseReal, unary_derivative: Some(deriv_sign) },
    FunctionDefinition { function: UnaryFunction::Gamma, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_gamma) },
    FunctionDefinition { function: UnaryFunction::Erf, arity: 1, branch: BranchPolicy::Principal, unary_derivative: Some(deriv_erf) },
];

fn unary(cc: &mut DomainExecutionContext<'_>, f: UnaryFunction, arg: TermId) -> TermId {
    cc.apply_semantic(SemanticOperator::from_unary(f), vec![arg])
}

fn deriv_exp(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    unary(cc, UnaryFunction::Exp, arg)
}

fn deriv_log(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    cc.apply_semantic(SemanticOperator::Power, vec![arg, cc.in_(-1)])
}

fn deriv_sin(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    unary(cc, UnaryFunction::Cos, arg)
}

fn deriv_cos(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let sin = unary(cc, UnaryFunction::Sin, arg);
    cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), sin])
}

fn deriv_tan(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let cos = unary(cc, UnaryFunction::Cos, arg);
    cc.apply_semantic(SemanticOperator::Power, vec![cos, cc.in_(-2)])
}

fn deriv_sinh(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    unary(cc, UnaryFunction::Cosh, arg)
}

fn deriv_cosh(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    unary(cc, UnaryFunction::Sinh, arg)
}

fn deriv_tanh(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let cosh = unary(cc, UnaryFunction::Cosh, arg);
    cc.apply_semantic(SemanticOperator::Power, vec![cosh, cc.in_(-2)])
}

fn deriv_arcsin(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let u2 = cc.apply_semantic(SemanticOperator::Power, vec![arg, cc.in_(2)]);
    let one_minus =
        cc.apply_semantic(SemanticOperator::Add, vec![cc.in_(1), cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), u2])]);
    let sqrt = cc.apply_semantic(SemanticOperator::Sqrt, vec![one_minus]);
    cc.apply_semantic(SemanticOperator::Power, vec![sqrt, cc.in_(-1)])
}

fn deriv_arccos(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let s = deriv_arcsin(cc, arg);
    cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), s])
}

fn deriv_arctan(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let u2 = cc.apply_semantic(SemanticOperator::Power, vec![arg, cc.in_(2)]);
    let one_plus = cc.apply_semantic(SemanticOperator::Add, vec![cc.in_(1), u2]);
    cc.apply_semantic(SemanticOperator::Power, vec![one_plus, cc.in_(-1)])
}

fn deriv_sqrt(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let sqrt = cc.apply_semantic(SemanticOperator::Sqrt, vec![arg]);
    let two_sqrt = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(2), sqrt]);
    cc.apply_semantic(SemanticOperator::Power, vec![two_sqrt, cc.in_(-1)])
}

fn deriv_abs(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let abs = cc.apply_semantic(SemanticOperator::Abs, vec![arg]);
    let uinv = cc.apply_semantic(SemanticOperator::Power, vec![arg, cc.in_(-1)]);
    cc.apply_semantic(SemanticOperator::Multiply, vec![abs, uinv])
}

fn deriv_sign(cc: &mut DomainExecutionContext<'_>, _arg: TermId) -> TermId {
    cc.in_(0)
}

fn deriv_gamma(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let gamma = unary(cc, UnaryFunction::Gamma, arg);
    let poly = cc.apply_semantic(SemanticOperator::PolyGamma, vec![cc.in_(0), arg]);
    cc.apply_semantic(SemanticOperator::Multiply, vec![gamma, poly])
}

fn deriv_erf(cc: &mut DomainExecutionContext<'_>, arg: TermId) -> TermId {
    let pi = cc.math_constant(athena_ir::MathematicalConstant::Pi);
    let sqrt_pi = cc.apply_semantic(SemanticOperator::Sqrt, vec![pi]);
    let inv = cc.apply_semantic(SemanticOperator::Power, vec![sqrt_pi, cc.in_(-1)]);
    let u2 = cc.apply_semantic(SemanticOperator::Power, vec![arg, cc.in_(2)]);
    let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), u2]);
    let exp = unary(cc, UnaryFunction::Exp, neg);
    cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(2), inv, exp])
}
