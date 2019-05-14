//! `interp/` 执行层 — KernelIR 编译 + 栈式 VM（Living `25` L2）。
//!
//! 符号树只在编译期遍历一次；运行期唯一执行形态是线性 [`ExecUnit`]。
//! 符号规则改写（Own / Delayed / DownValues）是语义数据操作，位于 [`rewrite`]；
//! 求值驱动、分派与控制流全部落在编译单元与 VM。

pub mod arith;
pub mod builtin;
pub mod control;
pub mod domain;
pub mod env;
pub mod index;
pub mod iterate;
pub mod kernel;
pub mod lin;
pub mod lower;
pub mod pattern;
pub mod rewrite;
pub mod vm;

use athena_numeric::Number;
use athena_types::{ComputationStatus, Diagnostic, Severity, SymbolId, TermId};

pub use env::{DefinitionLayer, LocalBinding, ScopeFrame};
pub use kernel::{ExecUnit, HandlerId, Instr};

/// handler 统一签名：接收已求值或原始操作数（由指令决定），返回结果。
pub(crate) type HandlerFn = fn(&mut vm::Vm<'_>, &[TermId]) -> Outcome;

/// `TermId` 版求值出口。
#[derive(Debug)]
pub struct Outcome {
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

impl Outcome {
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
pub fn push_number(session: &mut crate::session::Session, n: Number) -> TermId {
    let span = athena_ir::TermKind::default_span();
    session.arena.push(athena_ir::TermKind::Atom(athena_ir::AtomKind::Number(n)), span)
}

/// 会话级数字原子读取。
pub fn number_of<'a>(session: &'a crate::session::Session, id: TermId) -> Option<&'a Number> {
    match session.arena.get(id) {
        Some(athena_ir::TermKind::Atom(athena_ir::AtomKind::Number(n))) => Some(n),
        _ => None,
    }
}

/// 会话级符号替换（`Table` / `For` / `Function` 具化）。
pub fn substitute_symbol(session: &mut crate::session::Session, expr: TermId, sym: SymbolId, value: TermId) -> TermId {
    let mut vm = vm::Vm::new(session);
    pattern::substitute_symbol(&mut vm, expr, sym, value)
}

/// handler 表下标（与 [`HANDLERS`] 顺序一一对应）。
pub(crate) mod ids {
    use super::HandlerId;
    pub const PLUS: HandlerId = HandlerId(0);
    pub const TIMES: HandlerId = HandlerId(1);
    pub const POWER: HandlerId = HandlerId(2);
    pub const SUBTRACT: HandlerId = HandlerId(3);
    pub const DIVIDE: HandlerId = HandlerId(4);
    pub const DOT_TIMES: HandlerId = HandlerId(5);
    pub const DOT_DIVIDE: HandlerId = HandlerId(6);
    pub const DOT_POWER: HandlerId = HandlerId(7);
    pub const MLDIVIDE: HandlerId = HandlerId(8);
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
    pub const SET_EVAL_RHS: HandlerId = HandlerId(30);
    pub const SPAN: HandlerId = HandlerId(31);
    pub const PART: HandlerId = HandlerId(32);
    pub const APPLY: HandlerId = HandlerId(33);
    pub const REPLACE_ALL: HandlerId = HandlerId(34);
    pub const MAP: HandlerId = HandlerId(35);
    pub const COMPOUND: HandlerId = HandlerId(36);
    pub const COMPOUND_FRESH: HandlerId = HandlerId(37);
    pub const IF: HandlerId = HandlerId(38);
    pub const WHICH: HandlerId = HandlerId(39);
    pub const WHILE: HandlerId = HandlerId(40);
    pub const WHILE_FRESH: HandlerId = HandlerId(41);
    pub const FOR: HandlerId = HandlerId(42);
    pub const FOR_FRESH: HandlerId = HandlerId(43);
    pub const TRY: HandlerId = HandlerId(44);
    pub const WITH: HandlerId = HandlerId(45);
    pub const WITH_TOP: HandlerId = HandlerId(46);
    pub const MODULE: HandlerId = HandlerId(47);
    pub const MODULE_TOP: HandlerId = HandlerId(48);
    pub const BLOCK: HandlerId = HandlerId(49);
    pub const BLOCK_TOP: HandlerId = HandlerId(50);
    pub const HOLD: HandlerId = HandlerId(51);
    pub const PATTERN_HOLD: HandlerId = HandlerId(52);
    pub const TABLE: HandlerId = HandlerId(53);
    pub const SUM: HandlerId = HandlerId(54);
    pub const PRODUCT: HandlerId = HandlerId(55);
    pub const MATCH_Q: HandlerId = HandlerId(56);
    pub const CASES: HandlerId = HandlerId(57);
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

/// 全部内建 handler（下标 = [`HandlerId`] · 与 [`ids`] 一一对应）。
pub(crate) static HANDLERS: &[HandlerFn] = &[
    // 0.. arith
    arith::h_plus,
    arith::h_times,
    arith::h_power,
    arith::h_subtract,
    arith::h_divide,
    arith::h_dot_times,
    arith::h_dot_divide,
    arith::h_dot_power,
    arith::h_mldivide,
    // 9.. builtin comparisons / logic
    builtin::h_equal,
    builtin::h_unequal,
    builtin::h_less_chain,
    builtin::h_greater_chain,
    builtin::h_less_equal_chain,
    builtin::h_greater_equal_chain,
    builtin::h_and,
    builtin::h_or,
    builtin::h_not,
    builtin::h_list,
    builtin::h_unary_trig,
    builtin::h_sqrt,
    builtin::h_abs,
    builtin::h_factorial,
    builtin::h_simplify,
    builtin::h_range,
    builtin::h_length,
    builtin::h_first,
    builtin::h_join,
    builtin::h_unsupported,
    builtin::h_error,
    builtin::h_set_eval_rhs,
    // 31.. index
    index::h_span,
    index::h_part,
    index::h_apply,
    index::h_replace_all,
    index::h_map,
    // 36.. control
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
    // 53.. iterate
    iterate::h_table,
    iterate::h_sum,
    iterate::h_product,
    iterate::h_match_q,
    iterate::h_cases,
    iterate::h_function_rebuild,
    // 59.. lin
    lin::h_zeros,
    lin::h_ones,
    lin::h_eye,
    lin::h_size,
    lin::h_det,
    lin::h_linear_solve,
    lin::h_solve,
    // 66.. domain
    domain::h_calc_d,
    domain::h_calc_integrate,
    domain::h_calc_limit,
    domain::h_calc_series,
    domain::h_calc_dsolve,
    domain::h_calc_laplace,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handler_table_matches_ids() {
        assert_eq!(HANDLERS.len(), 72, "HANDLERS 与 ids 常量必须一一对应");
    }
}
