//! 执行引擎句柄 — 宿主组合请求；数学逻辑在子模块中。

use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId, TermId};

use crate::{
    api::request::{AthenaRequest, LoweringOutcome},
    domains::dispatch::{DomainRequest, DomainResult},
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
    /// 创建引擎句柄。
    pub fn new() -> Self {
        Self {}
    }

    /// 在内建定义下求值（唯一 `ExecutionIR` 路径）。执行失败经 [`Result`] 传播，禁止吞错。
    pub fn evaluate(&self, session: &mut Session, term: TermId) -> Result<TermId> {
        let result_id = execution::execute_ir_request(session, AthenaRequest::Term(term))?;
        session.results.require_symbolic_term(result_id)
    }

    /// 先求导再求值（session arena · 求导后走 `ExecutionIR`）。执行失败经 [`Result`] 传播，禁止吞错。
    pub fn differentiate(&self, session: &mut Session, term: TermId, var: &str) -> Result<TermId> {
        let mut dc = crate::domains::DomainExecutionContext::new(session);
        let var = dc.intern(var);
        let d = crate::domains::calculus::differentiate(&mut dc, term, var)?;
        self.evaluate(session, d)
    }

    /// 域请求经 语义入口（[`Session::mgraph`] → Reflector → provider）。
    ///
    /// Provider-only 分派仍在 [`crate::domains::dispatch::execute_domain`]，供
    /// `NeedComputation` / ExecutionIR `CallProvider` 内部使用。
    pub fn execute_domain(&self, session: &mut Session, request: DomainRequest) -> Result<DomainResult> {
        crate::reasoning::mgraph::execute_domain_via_semantic_entry(session, request)
    }

    /// 语义入口：`DomainGoal` → Obligation → Reflector → Plan / Result（绑定 session M-Graph）。
    pub fn execute_domain_goal(
        &self,
        session: &mut Session,
        goal: crate::api::request::DomainGoal,
    ) -> Result<crate::reasoning::mgraph::DomainSemanticOutcome> {
        crate::reasoning::mgraph::execute_domain_goal(session, goal)
    }

    /// 经中性 [`AthenaRequest`] 边界执行。
    ///
    /// 唯一路径：[`execution::execute_ir_request`]（含 `Goal::Dispatch` → `CallProvider`）。
    pub fn execute_request(&self, session: &mut Session, request: AthenaRequest) -> Result<ResultId> {
        execution::execute_ir_request(session, request)
    }

    /// 将方言 [`LoweringOutcome`] 送入后端（Rejected 直接返回诊断）。
    pub fn execute_lowering_outcome(&self, session: &mut Session, outcome: LoweringOutcome) -> Result<ResultId> {
        match outcome {
            LoweringOutcome::Accepted(request) => self.execute_request(session, request),
            LoweringOutcome::Rejected(diagnostic) => Err(diagnostic),
        }
    }

    /// 经 `SemanticOperator::Simplify` 化简（唯一 `ExecutionIR` 路径）。
    ///
    /// 缺少符号项投影时硬失败，禁止回落到输入项。
    pub fn simplify(&self, session: &mut Session, term: TermId) -> Result<TermId> {
        let wrapped = execution::push_semantic(session, athena_ir::SemanticOperator::Simplify, vec![term]);
        let result_id = execution::execute_ir_request(session, AthenaRequest::Term(wrapped))?;
        session.results.require_symbolic_term(result_id)
    }

    /// 占位：无 arena 的桩求值（正式路径请用 [`Self::evaluate`] / [`Self::execute_request`]）。
    pub fn evaluate_unit(&self, _term: &(), _opts: &EvalOptions) -> Result<()> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation))
    }
}
