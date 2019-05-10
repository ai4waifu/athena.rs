//! 执行引擎句柄 — 宿主组合请求；数学逻辑在子模块中。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    domain::{DomainRequest, DomainResult, execute_domain as dispatch_domain},
    numeric_clone::clone_term,
    term::Term,
};

/// 求值选项（占位；随后随模式 / Session 扩展）。
#[derive(Debug, Default)]
pub struct EvalOptions {}

/// 化简选项。
#[derive(Debug, Default)]
pub struct SimplifyOptions {}

/// Athena 主引擎句柄（无状态规则；绑定请用 [`Session`]）。
#[derive(Debug, Default)]
pub struct AthenaEngine {}

impl AthenaEngine {
    /// 以默认算子注册表创建引擎（桩）。
    pub fn new() -> Self {
        Self {}
    }

    /// 在内建定义下求值桥接 [`Term`]。
    pub fn evaluate_term(&self, expr: &Term) -> Term {
        crate::eval::evaluate(expr)
    }

    /// 先求导再求值（遗留桥接；优先使用 [`Self::execute_domain`]）。
    pub fn differentiate_term(&self, expr: &Term, var: &str) -> Term {
        crate::eval::evaluate(&crate::calculus::differentiate(expr, var))
    }

    /// 域分派 — 返回按域区分的 [`DomainResult`]。
    pub fn execute_domain(&self, request: DomainRequest) -> Result<DomainResult> {
        dispatch_domain(request)
    }

    /// 经 `Simplify` 头部化简。
    pub fn simplify_term(&self, expr: &Term) -> Term {
        self.evaluate_term(&Term::apply("Simplify", vec![clone_term(expr)]))
    }

    /// Arena/`()` 桩求值 — 保留至 IR 路径落地。
    pub fn evaluate(&self, _term: &(), _opts: &EvalOptions) -> Result<()> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
    }
}
