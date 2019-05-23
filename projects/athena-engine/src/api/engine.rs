//! 执行引擎句柄 — 宿主组合请求；数学逻辑在子模块中。

use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use crate::{
    domains::dispatch::{DomainRequest, DomainResult, execute_domain as dispatch_domain},
    execution,
    runtime::session::Session,
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

    /// 在内建定义下求值（KernelIR + VM · Living `25` L4）。
    pub fn evaluate_expression(&self, session: &mut Session, expr: TermId) -> TermId {
        execution::vm::evaluate_session(session, expr).term
    }

    /// 先求导再求值（session arena · Living `25`）。
    pub fn differentiate_expression(&self, session: &mut Session, expr: TermId, var: &str) -> TermId {
        let d =
            crate::domains::calculus::differentiate(&mut crate::domains::calculus::ctx::CalculusCtx::new(session), expr, var);
        execution::vm::evaluate_session(session, d).term
    }

    /// 域分派 — 返回按域区分的 [`DomainResult`]。
    pub fn execute_domain(&self, session: &mut Session, request: DomainRequest) -> Result<DomainResult> {
        dispatch_domain(session, request)
    }

    /// 经 `Simplify` 头部化简（KernelIR + VM）。
    pub fn simplify_expression(&self, session: &mut Session, expr: TermId) -> TermId {
        let wrapped = execution::push_app(session, "Simplify", vec![expr]);
        execution::vm::evaluate_session(session, wrapped).term
    }

    /// Arena/`()` 桩求值 — 保留至 IR 路径落地。
    pub fn evaluate(&self, _term: &(), _opts: &EvalOptions) -> Result<()> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
    }
}
