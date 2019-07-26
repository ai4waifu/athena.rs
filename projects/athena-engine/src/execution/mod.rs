//! 执行层 — typed [`ir::ExecutionIR`] + backends（Living `25` 终态）。
//!
//! 合同：[`compiler`] · [`ir`] · [`reference`] · [`backend`] · [`provider`]。
//! Pattern 工具在 [`builtins::patterns`]。

pub mod backend;
pub mod builtins;
pub mod compiler;
pub mod environment;
pub mod ir;
pub mod provider;
pub mod reference;
pub(crate) mod shape;

use athena_numeric::Number;
use athena_types::{ComputationStatus, Diagnostic, Result as AthenaResult, ResultId, Severity, SymbolId, TermId};

use crate::{api::request::AthenaRequest, runtime::session::Session};

pub use environment::{DefinitionLayer, LocalBinding, ScopeFrame};

/// Compile and run one request on the `ExecutionIR` path only.
///
/// `Goal::Dispatch` carries the `DomainRequest` into `CallProvider` at runtime.
pub fn execute_ir_request(session: &mut Session, request: AthenaRequest) -> AthenaResult<ResultId> {
    use crate::api::request::DomainGoal;

    let module = compiler::ExecutionCompiler::new().compile(session, &request)?;
    let domain = match request {
        AthenaRequest::Goal(DomainGoal::Dispatch(domain)) => Some(domain),
        _ => None,
    };
    reference::ReferenceExecutor::new().execute(session, &module, domain)
}

/// Project a term through [`execute_ir_request`] into a compact [`TermEvaluation`].
///
/// Test / internal helper only — not a second execution model. Product paths use
/// [`execute_ir_request`] or [`crate::api::AthenaEngine::execute_request`].
pub fn evaluate_term(session: &mut Session, expr: TermId) -> TermEvaluation {
    match execute_ir_request(session, AthenaRequest::Term(expr)) {
        Ok(result_id) => {
            let Some(result) = session.results.get(result_id)
            else {
                return TermEvaluation::unevaluated(expr);
            };
            let term = result.symbolic_term.unwrap_or(expr);
            let diagnostics = result.diagnostics.clone();
            let status = result.status;
            let has_error = diagnostics.iter().any(|d| d.severity == Severity::Error);
            let kind = if status == ComputationStatus::Exact && !has_error { EvalKind::Value } else { EvalKind::Unevaluated };
            TermEvaluation { term, kind, status, diagnostics }
        }
        Err(diagnostic) => TermEvaluation::invalid(expr, diagnostic),
    }
}

/// Term 求值内部报告（非正式公共 `ComputationResult`）。
#[derive(Debug)]
pub struct TermEvaluation {
    /// 结果项（失败时可为原式或保守回显）。
    pub term: TermId,
    /// 值 / 未求值区分。
    pub kind: EvalKind,
    /// 统一计算状态。
    pub status: ComputationStatus,
    /// 结构化诊断。
    pub diagnostics: Vec<Diagnostic>,
}

/// 值 / 未求值区分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalKind {
    /// 已归约为正常值。
    Value,
    /// 保留未求值形式，不得冒充成功 exact。
    Unevaluated,
}

impl TermEvaluation {
    /// 精确值出口。
    pub fn value(term: TermId) -> Self {
        Self { term, kind: EvalKind::Value, status: ComputationStatus::Exact, diagnostics: Vec::new() }
    }

    /// 未求值保留。
    pub fn unevaluated(term: TermId) -> Self {
        Self { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Unknown, diagnostics: Vec::new() }
    }

    /// 硬失败：带 Error 诊断，状态 [`ComputationStatus::Invalid`]。
    pub fn invalid(term: TermId, diagnostic: Diagnostic) -> Self {
        Self { term, kind: EvalKind::Unevaluated, status: ComputationStatus::Invalid, diagnostics: vec![diagnostic] }
    }

    /// 是否含 Error 级诊断。
    pub fn has_error(&self) -> bool {
        self.diagnostics.iter().any(|d| d.severity == Severity::Error)
    }

    /// 合并诊断（Error 提级为 Invalid / Unevaluated）。
    pub fn with_diagnostics(mut self, mut diagnostics: Vec<Diagnostic>) -> Self {
        if diagnostics.is_empty() {
            return self;
        }
        diagnostics.append(&mut self.diagnostics);
        self.diagnostics = diagnostics;
        if self.diagnostics.iter().any(|d| d.severity == Severity::Error) {
            self.status = ComputationStatus::Invalid;
            self.kind = EvalKind::Unevaluated;
        }
        self
    }
}

/// 会话级数字原子构造。
pub fn push_number(session: &mut crate::runtime::session::Session, n: Number) -> TermId {
    let span = athena_ir::TermNode::default_span();
    session.arena.push(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n)), span)
}

/// 会话级 App 构造（算子名 intern）。
pub fn push_application(session: &mut crate::runtime::session::Session, head: &str, args: Vec<TermId>) -> TermId {
    crate::runtime::values::arena::push_application_named(session, head, args)
}

/// 会话级数字原子读取。
pub fn number_of<'a>(session: &'a crate::runtime::session::Session, id: TermId) -> Option<&'a Number> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => Some(n),
        _ => None,
    }
}

/// 会话级符号替换（`Table` / `CountedLoop` / `Function` 具化）。
pub fn substitute_symbol(session: &mut crate::runtime::session::Session, expr: TermId, symbol: SymbolId, value: TermId) -> TermId {
    builtins::patterns::substitute_symbol(session, expr, symbol, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::request::AthenaRequest;

    #[test]
    fn execute_ir_request_atom_term() {
        let mut session = Session::new();
        let term = session.builder().int(4, Default::default());
        let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("ir");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(term));
    }
}
