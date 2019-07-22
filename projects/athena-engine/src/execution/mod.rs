//! 执行层 — 终态为 typed [`ir::ExecutionIR`] + backends。
//!
//! 合同已冻结：[`compiler`] · [`ir`] · [`reference`] · [`backend`] · [`provider`]。
//! 旧 `compile` / `kernel_ir` / `vm` 路径仍暂存于树中，cutover 时整段删除，禁止
//! 适配层或双执行入口。符号规则改写仍位于 [`builtins::rewriting`]。

pub mod backend;
pub mod builtins;
pub(crate) mod compile;
pub mod compiler;
pub mod environment;
pub mod ir;
pub(crate) mod kernel_ir;
pub mod provider;
pub mod reference;
pub(crate) mod shape;
pub mod vm;

use athena_numeric::Number;
use athena_types::{ComputationStatus, Diagnostic, Result as AthenaResult, ResultId, Severity, SymbolId, TermId};

use crate::api::request::AthenaRequest;
use crate::runtime::session::Session;

pub use environment::{DefinitionLayer, LocalBinding, ScopeFrame};
pub(crate) use kernel_ir::{ExecUnit, HandlerId, Instr};

/// Compile and run one request on the `ExecutionIR` path only.
///
/// This is the cutover entry for backends. It must not call the stack VM.
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

/// Session 顶层求值入口（语句语义 · Living `25` L2 公共门）。
///
/// Cutover：经 [`execute_ir_request`]，不再走栈式 VM。
/// 仍返回 [`TermEvaluation`] 以兼容尚未迁完的验收测试。
pub fn evaluate_session(session: &mut Session, expr: TermId) -> TermEvaluation {
    match execute_ir_request(session, AthenaRequest::Term(expr)) {
        Ok(result_id) => {
            let Some(result) = session.results.get(result_id) else {
                return TermEvaluation::unevaluated(expr);
            };
            let term = result.symbolic_term.unwrap_or(expr);
            let diagnostics = result.diagnostics.clone();
            let status = result.status;
            let has_error = diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error);
            let kind = if status == ComputationStatus::Exact && !has_error {
                EvalKind::Value
            } else {
                EvalKind::Unevaluated
            };
            TermEvaluation {
                term,
                kind,
                status,
                diagnostics,
            }
        }
        Err(diagnostic) => TermEvaluation::invalid(expr, diagnostic),
    }
}

/// handler 统一签名：接收已求值或原始操作数（由指令决定），返回结果。
pub(crate) type HandlerFn = fn(&mut vm::Vm<'_>, &[TermId]) -> TermEvaluation;

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

/// handler 表下标（与 [`HANDLERS`] 顺序一一对应）。
pub(crate) mod ids {
    use crate::execution::HandlerId;
    pub const PLUS: HandlerId = HandlerId(0);
    pub const TIMES: HandlerId = HandlerId(1);
    pub const POWER: HandlerId = HandlerId(2);
    pub const SUBTRACT: HandlerId = HandlerId(3);
    pub const DIVIDE: HandlerId = HandlerId(4);
    pub const DOT_TIMES: HandlerId = HandlerId(5);
    pub const DOT_DIVIDE: HandlerId = HandlerId(6);
    pub const DOT_POWER: HandlerId = HandlerId(7);
    /// 预留下标（原 MATLAB `\` 表面槽）。求解走 [`LINEAR_SOLVE`]。
    pub const RESERVED_08: HandlerId = HandlerId(8);
    pub const EQUAL: HandlerId = HandlerId(9);
    pub const UNEQUAL: HandlerId = HandlerId(10);
    pub const LESS_CHAIN: HandlerId = HandlerId(11);
    pub const GREATER_CHAIN: HandlerId = HandlerId(12);
    pub const LESS_EQUAL_CHAIN: HandlerId = HandlerId(13);
    pub const GREATER_EQUAL_CHAIN: HandlerId = HandlerId(14);
    pub const AND: HandlerId = HandlerId(15);
    pub const OR: HandlerId = HandlerId(16);
    pub const NOT: HandlerId = HandlerId(17);
    pub const LIST: HandlerId = HandlerId(18);
    pub const UNARY_TRIG: HandlerId = HandlerId(19);
    pub const SQRT: HandlerId = HandlerId(20);
    pub const ABS: HandlerId = HandlerId(21);
    pub const FACTORIAL: HandlerId = HandlerId(22);
    pub const SIMPLIFY: HandlerId = HandlerId(23);
    pub const RANGE: HandlerId = HandlerId(24);
    pub const LENGTH: HandlerId = HandlerId(25);
    pub const FIRST: HandlerId = HandlerId(26);
    pub const JOIN: HandlerId = HandlerId(27);
    pub const UNSUPPORTED: HandlerId = HandlerId(28);
    pub const ERROR: HandlerId = HandlerId(29);
    pub const DEFINE_EVAL_RHS: HandlerId = HandlerId(30);
    pub const SPAN: HandlerId = HandlerId(31);
    pub const PART: HandlerId = HandlerId(32);
    pub const APPLY: HandlerId = HandlerId(33);
    pub const REPLACE_ALL: HandlerId = HandlerId(34);
    pub const MAP: HandlerId = HandlerId(35);
    pub const SEQUENCE: HandlerId = HandlerId(36);
    pub const SEQUENCE_FRESH: HandlerId = HandlerId(37);
    pub const BRANCH: HandlerId = HandlerId(38);
    pub const COND: HandlerId = HandlerId(39);
    pub const LOOP_WHILE: HandlerId = HandlerId(40);
    pub const LOOP_WHILE_FRESH: HandlerId = HandlerId(41);
    pub const COUNTED_LOOP: HandlerId = HandlerId(42);
    pub const COUNTED_LOOP_FRESH: HandlerId = HandlerId(43);
    pub const RECOVER: HandlerId = HandlerId(44);
    pub const LOCAL_SCOPE: HandlerId = HandlerId(45);
    pub const LOCAL_SCOPE_TOP: HandlerId = HandlerId(46);
    pub const LEXICAL_SCOPE: HandlerId = HandlerId(47);
    pub const LEXICAL_SCOPE_TOP: HandlerId = HandlerId(48);
    pub const DYNAMIC_SCOPE: HandlerId = HandlerId(49);
    pub const DYNAMIC_SCOPE_TOP: HandlerId = HandlerId(50);
    pub const HOLD: HandlerId = HandlerId(51);
    pub const PATTERN_HOLD: HandlerId = HandlerId(52);
    pub const TABLE: HandlerId = HandlerId(53);
    pub const SUM: HandlerId = HandlerId(54);
    pub const PRODUCT: HandlerId = HandlerId(55);
    pub const MATCHES: HandlerId = HandlerId(56);
    pub const COLLECT_MATCHES: HandlerId = HandlerId(57);
    pub const FUNCTION_REBUILD: HandlerId = HandlerId(58);
    pub const ZEROS: HandlerId = HandlerId(59);
    pub const ONES: HandlerId = HandlerId(60);
    pub const EYE: HandlerId = HandlerId(61);
    pub const SIZE: HandlerId = HandlerId(62);
    pub const DET: HandlerId = HandlerId(63);
    pub const LINEAR_SOLVE: HandlerId = HandlerId(64);
    pub const SOLVE: HandlerId = HandlerId(65);
    pub const CALC_D: HandlerId = HandlerId(66);
    pub const CALC_INTEGRATE: HandlerId = HandlerId(67);
    pub const CALC_LIMIT: HandlerId = HandlerId(68);
    pub const CALC_SERIES: HandlerId = HandlerId(69);
    pub const CALC_DSOLVE: HandlerId = HandlerId(70);
    pub const CALC_LAPLACE: HandlerId = HandlerId(71);
}

use builtins::{arithmetic, catalog, control, domains, indexing, iteration, matrix};

/// 全部内建 handler（下标 = [`HandlerId`] · 与 [`ids`] 一一对应）。
pub(crate) static HANDLERS: &[HandlerFn] = &[
    arithmetic::h_plus,
    arithmetic::h_times,
    arithmetic::h_power,
    arithmetic::h_subtract,
    arithmetic::h_divide,
    arithmetic::h_dot_times,
    arithmetic::h_dot_divide,
    arithmetic::h_dot_power,
    catalog::h_reserved,
    catalog::h_equal,
    catalog::h_unequal,
    catalog::h_less_chain,
    catalog::h_greater_chain,
    catalog::h_less_equal_chain,
    catalog::h_greater_equal_chain,
    catalog::h_and,
    catalog::h_or,
    catalog::h_not,
    catalog::h_list,
    catalog::h_unary_trig,
    catalog::h_sqrt,
    catalog::h_abs,
    catalog::h_factorial,
    catalog::h_simplify,
    catalog::h_range,
    catalog::h_length,
    catalog::h_first,
    catalog::h_join,
    catalog::h_unsupported,
    catalog::h_error,
    catalog::h_define_eval_rhs,
    indexing::h_span,
    indexing::h_part,
    indexing::h_apply,
    indexing::h_replace_all,
    indexing::h_map,
    control::h_compound,
    control::h_compound_fresh,
    control::h_if,
    control::h_which,
    control::h_while,
    control::h_while_fresh,
    control::h_for,
    control::h_for_fresh,
    control::h_try,
    control::h_with,
    control::h_with_top,
    control::h_module,
    control::h_module_top,
    control::h_block,
    control::h_block_top,
    control::h_hold,
    control::h_pattern_hold,
    iteration::h_table,
    iteration::h_sum,
    iteration::h_product,
    iteration::h_match_q,
    iteration::h_cases,
    iteration::h_function_rebuild,
    matrix::h_zeros,
    matrix::h_ones,
    matrix::h_eye,
    matrix::h_size,
    matrix::h_det,
    matrix::h_linear_solve,
    matrix::h_solve,
    domains::h_calc_d,
    domains::h_calc_integrate,
    domains::h_calc_limit,
    domains::h_calc_series,
    domains::h_calc_dsolve,
    domains::h_calc_laplace,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::request::AthenaRequest;

    #[test]
    fn handler_table_matches_ids() {
        assert_eq!(HANDLERS.len(), 72, "HANDLERS 与 ids 常量必须一一对应");
    }

    #[test]
    fn execute_ir_request_atom_term() {
        let mut session = Session::new();
        let term = session.builder().int(4, Default::default());
        let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("ir");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(term));
    }
}
