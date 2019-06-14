//! KernelIR 执行单元 — 线性指令合同（Living `25` L2）。
//!
//! 符号树只在编译期遍历一次；运行期唯一执行形态是 [`ExecUnit`]。
//! 指令为后缀式：操作数走值栈，原始操作数（lower 期已知）内嵌 [`TermId`]。

use athena_types::{OperatorId, TermId};

/// 内建 handler 表下标（分派预解析产物）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandlerId(pub u32);

/// KernelIR 指令。
#[derive(Debug, Clone)]
pub enum Instr {
    /// 压入编译期已确定的子树引用（arena 共享，不复制）。
    Constant {
        /// 常量子树根。
        term: TermId,
    },
    /// 弹出 `argc` 个元素构造 `List`。
    MakeList {
        /// 元素个数。
        argc: u16,
    },
    /// 弹出 `argc` 个已求值参数，按 `op` 惰性重建 `Application`（未知算子 · Unevaluated）。
    MakeApplication {
        /// 算子。
        op: OperatorId,
        /// 参数个数。
        argc: u16,
    },
    /// 弹出 `argc` 个已求值参数调用预解析 handler。
    EvalOp {
        /// 预解析 handler。
        handler: HandlerId,
        /// 参数个数。
        argc: u16,
    },
    /// 以内嵌原始操作数（不求值）调用 handler；handler 自行选择重入求值。
    EvalRaw {
        /// 预解析 handler。
        handler: HandlerId,
        /// 原始操作数。
        operands: Vec<TermId>,
    },
    /// 弹出 `argc` 个已求值参数与已求值 head 值（`Function` 应用 / 惰性重建）。
    EvalDynamic {
        /// 参数个数。
        argc: u16,
    },
    /// 条件跳转：弹出栈顶，typed false 时跳转（短路控制流预留）。
    BranchFalse {
        /// 跳转目标。
        target: u32,
    },
    /// 无条件跳转（短路控制流预留）。
    Jump {
        /// 跳转目标。
        target: u32,
    },
    /// 语句层定义：弹出 rhs 写入当前 env，栈顶保留 rhs（立即定义语句位）。
    DefineOwn {
        /// 定义符号。
        symbol: athena_types::SymbolId,
    },
    /// 语句层定义：弹出 rhs 存 Delayed，压入 `Null`（延迟定义语句位）。
    DefineDelayed {
        /// 定义符号。
        symbol: athena_types::SymbolId,
    },
    /// 语句层定义：弹出 rhs 与内嵌 lhs 模式存 DownValues，压入 `Null`（延迟 DownValue 语句位）。
    DefineDownValue {
        /// 定义符号。
        symbol: athena_types::SymbolId,
        /// lhs 模式子树。
        lhs: TermId,
    },
    /// 结束单元，栈顶为单元值。
    Return,
}

/// 编译产物：一个 canonical 结构对应的线性指令序列。
#[derive(Debug, Clone)]
pub struct ExecUnit {
    /// 编译源子树（缓存命中时作 `structural_eq` 复核基准）。
    pub source: TermId,
    /// 线性指令。
    pub code: Vec<Instr>,
}

impl ExecUnit {
    /// 常量单元（仅返回原子子树）。
    pub fn constant(source: TermId) -> Self {
        Self { source, code: vec![Instr::Constant { term: source }, Instr::Return] }
    }
}
