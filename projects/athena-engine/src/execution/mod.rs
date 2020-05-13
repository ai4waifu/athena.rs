//! 执行层 — typed [`ir::ExecutionIR`] + backends（终态）。
//!
//! 合同：[`compiler`] · [`ir`] · [`reference`] · [`backend`] · [`provider`] · [`vm`]。
//! Pattern 工具在 [`builtins::patterns`]。
//!
//! `athena-vm` 提供受限解释运行时骨架。完整 SSA `ExecutionModule` 仍在本 crate，
//! 经 [`vm`] 投影配置。禁止 VM 拥有持久数学 payload 或前端字符串分派。

pub mod backend;
pub mod builtins;
pub mod compiler;
pub mod environment;
pub mod execution_host;
pub mod ir;
pub mod provider;
pub mod reference;
pub mod vm;
pub mod vm_lower;
pub(crate) mod shape;

use athena_numeric::Number;
use athena_types::{ComputationStatus, Diagnostic, Result as AthenaResult, ResultId, Severity, SymbolId, TermId};

use crate::{api::request::AthenaRequest, runtime::session::Session};

pub use environment::{CompiledRuleStore, DefinitionLayer, LocalBinding, ScopeFrame};

/// 仅在 `ExecutionIR` 路径上编译并执行一次请求。
///
/// `Goal::Dispatch` 在运行时把 `DomainRequest` 带入 `CallProvider`。
/// 后端经 [`backend::select_execution_backend`] **显式选择**：选中 `AthenaVm` 时只走 VM，
/// 失败返回诊断，**禁止**再静默回退 `ReferenceExecutor`。
pub fn execute_ir_request(session: &mut Session, request: AthenaRequest) -> AthenaResult<ResultId> {
    use crate::api::request::DomainGoal;
    use crate::execution::backend::{BackendKind, select_execution_backend};
    use crate::runtime::results::{ComputationResult, CoverageStatus, ResultProvenance};
    use athena_types::{Diagnostic, DiagnosticCode};
    use athena_vm::SlotValue;

    let module = compiler::ExecutionCompiler::new().compile(session, &request)?;
    let domain = match request {
        AthenaRequest::Goal(DomainGoal::Dispatch(domain)) => Some(domain),
        _ => None,
    };
    match select_execution_backend(&module, domain.is_some()) {
        BackendKind::AthenaVm => match vm::execute_verified_cfg_on_vm(session, &module) {
            Ok(SlotValue::Boolean(value)) => {
                let term = session.builder().boolean(value, Default::default());
                let value_id = session.insert_symbolic_value(term);
                let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
                    .with_value(value_id)
                    .with_symbolic_term(term)
                    .with_provenance(ResultProvenance::kind("ExecutionIR/athena-vm"));
                Ok(session.insert_result(result))
            }
            Ok(SlotValue::Term(term)) => {
                let term_ref = session.arena.term_ref(term).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "execute_ir_request")
                        .detail("backend", "athena-vm")
                        .detail("reason", "vm_term_out_of_range")
                })?;
                let term = session.arena.check_ref(term_ref)?;
                let value_id = session.insert_symbolic_value(term);
                let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
                    .with_value(value_id)
                    .with_symbolic_term(term)
                    .with_provenance(ResultProvenance::kind("ExecutionIR/athena-vm"));
                Ok(session.insert_result(result))
            }
            Ok(SlotValue::Symbol(symbol)) => {
                let term = session.builder().symbol_id(symbol, Default::default());
                let value_id = session.insert_symbolic_value(term);
                let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
                    .with_value(value_id)
                    .with_symbolic_term(term)
                    .with_provenance(ResultProvenance::kind("ExecutionIR/athena-vm"));
                Ok(session.insert_result(result))
            }
            Ok(SlotValue::Unit) => {
                let term = session.builder().null(Default::default());
                let value_id = session.insert_symbolic_value(term);
                let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
                    .with_value(value_id)
                    .with_symbolic_term(term)
                    .with_provenance(ResultProvenance::kind("ExecutionIR/athena-vm"));
                Ok(session.insert_result(result))
            }
            Ok(SlotValue::Scope(_)) => {
                let term = session.builder().null(Default::default());
                let value_id = session.insert_symbolic_value(term);
                let result = ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
                    .with_value(value_id)
                    .with_symbolic_term(term)
                    .with_provenance(ResultProvenance::kind("ExecutionIR/athena-vm"));
                Ok(session.insert_result(result))
            }
            Ok(_) => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "execute_ir_request")
                .detail("backend", "athena-vm")
                .detail("reason", "vm_unexpected_slot_kind")),
            Err(diagnostic) => Err(diagnostic
                .detail("component", "execute_ir_request")
                .detail("backend", "athena-vm")
                .detail("reason", "vm_backend_failed_no_fallback")),
        },
        BackendKind::Reference => reference::ReferenceExecutor::new().execute(session, &module, domain),
        other => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "execute_ir_request")
            .detail("reason", "backend_not_wired")
            .detail("backend", format!("{other:?}"))),
    }
}

/// 经 [`execute_ir_request`] 将项投影为紧凑的 [`TermEvaluation`]。
///
/// 仅供测试 / 内部辅助 — 不是第二套执行模型。产品路径使用
/// [`execute_ir_request`] 或 [`crate::api::AthenaEngine::execute_request`]。
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

/// 会话级 App 构造（core semantic）。
pub fn push_semantic(session: &mut crate::runtime::session::Session, op: athena_ir::SemanticOperator, args: Vec<TermId>) -> TermId {
    crate::runtime::values::arena::push_semantic(session, op, args)
}

/// 会话级 extension App 构造（[`ExtensionOperatorId`](athena_types::ExtensionOperatorId)，永不字符串→核心语义）。
pub fn push_extension(session: &mut crate::runtime::session::Session, op: athena_types::ExtensionOperatorId, args: Vec<TermId>) -> TermId {
    crate::runtime::values::arena::push_extension(session, op, args)
}

/// 会话级数字原子读取。
pub fn number_of<'a>(session: &'a crate::runtime::session::Session, id: TermId) -> Option<&'a Number> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => Some(n),
        _ => None,
    }
}

/// 会话级符号替换（迭代 / 作用域具化）。
pub fn substitute_symbol(session: &mut crate::runtime::session::Session, expr: TermId, symbol: SymbolId, value: TermId) -> TermId {
    builtins::patterns::substitute_symbol(session, expr, symbol, value)
}
