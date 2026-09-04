//! 执行引擎句柄 — 宿主组合请求；数学逻辑在子模块中。

use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId, TermId};

use crate::{
    api::request::{AthenaRequest, DomainGoal, LoweringOutcome},
    domains::dispatch::{DomainRequest, DomainResult, execute_domain as dispatch_domain},
    execution,
    runtime::{
        results::{ComputationResult, CoverageStatus, ResultProvenance},
        session::Session,
    },
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
    /// 创建引擎句柄。
    pub fn new() -> Self {
        Self {}
    }

    /// 在内建定义下求值（KernelIR + VM）。返回归约后的 [`TermId`]（内部投影，非正式公共结果）。
    pub fn evaluate(&self, session: &mut Session, term: TermId) -> TermId {
        execution::vm::evaluate_session(session, term).term
    }

    /// 先求导再求值（session arena）。
    pub fn differentiate(&self, session: &mut Session, term: TermId, var: &str) -> TermId {
        let d = crate::domains::calculus::differentiate(&mut crate::domains::calculus::ctx::CalculusCtx::new(session), term, var);
        execution::vm::evaluate_session(session, d).term
    }

    /// 域分派 — 返回按域区分的 [`DomainResult`]。
    pub fn execute_domain(&self, session: &mut Session, request: DomainRequest) -> Result<DomainResult> {
        dispatch_domain(session, request)
    }

    /// 经中性 [`AthenaRequest`] 边界执行（Living `26`）。
    ///
    /// - `Term`：现有 VM 求值，结果写入 [`crate::runtime::results::ResultStore`]
    /// - `Goal::Dispatch`：现有域分派，结果写入 ResultStore
    /// - `Command` / `Control`：本切片尚未接入，写入显式 unsupported 结果（禁止静默成功）
    pub fn execute_request(&self, session: &mut Session, request: AthenaRequest) -> Result<ResultId> {
        match request {
            AthenaRequest::Term(term) => {
                let outcome = execution::vm::evaluate_session(session, term);
                let value = session.insert_symbolic_value(outcome.term);
                let coverage = match outcome.kind {
                    execution::EvalKind::Value => CoverageStatus::Full,
                    execution::EvalKind::Unevaluated => CoverageStatus::Unknown,
                };
                let mut result = ComputationResult::with_status(outcome.status, coverage)
                    .with_value(value)
                    .with_symbolic_term(outcome.term)
                    .with_provenance(ResultProvenance { request_kind: "Term" });
                for diagnostic in outcome.diagnostics {
                    result = result.with_diagnostic(diagnostic);
                }
                Ok(session.insert_result(result))
            }
            AthenaRequest::Goal(DomainGoal::Dispatch(domain_request)) => {
                let domain_result = self.execute_domain(session, domain_request)?;
                let result = crate::runtime::results::computation_from_domain(session, domain_result);
                Ok(session.insert_result(result))
            }
            AthenaRequest::Command(_) | AthenaRequest::Control(_) => {
                let operation = request.kind_name();
                let diagnostic =
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("phase", "request_boundary").detail("operation", operation);
                let result = ComputationResult::with_status(athena_types::ComputationStatus::Invalid, CoverageStatus::Unsupported)
                    .with_diagnostic(diagnostic.clone())
                    .with_provenance(ResultProvenance { request_kind: operation });
                let _ = session.insert_result(result);
                Err(diagnostic)
            }
        }
    }

    /// 将方言 [`LoweringOutcome`] 送入后端（Rejected 直接返回诊断）。
    pub fn execute_lowering_outcome(&self, session: &mut Session, outcome: LoweringOutcome) -> Result<ResultId> {
        match outcome {
            LoweringOutcome::Accepted(request) => self.execute_request(session, request),
            LoweringOutcome::Rejected(diagnostic) => Err(diagnostic),
        }
    }

    /// 经 `Simplify` 头部化简（KernelIR + VM）。
    pub fn simplify(&self, session: &mut Session, term: TermId) -> TermId {
        let wrapped = execution::push_application(session, "Simplify", vec![term]);
        execution::vm::evaluate_session(session, wrapped).term
    }

    /// 占位：无 arena 的桩求值（正式路径请用 [`Self::evaluate`] / [`Self::execute_request`]）。
    pub fn evaluate_unit(&self, _term: &(), _opts: &EvalOptions) -> Result<()> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
    }
}
