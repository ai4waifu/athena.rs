//! 带显式块参数的基本块（无隐式栈参数）。

use super::{
    ids::{BlockId, SsaValueId},
    operation::Operation,
    terminator::Terminator,
    types::ExecutionValueType,
};

/// 一个 SSA 基本块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    /// 块标识。
    pub id: BlockId,
    /// 形式参数（块参数）。
    pub parameters: Vec<BlockParameter>,
    /// 有序 SSA 操作。
    pub operations: Vec<Operation>,
    /// 唯一终结器。
    pub terminator: Terminator,
}

/// 带类型的块参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockParameter {
    /// 在块入口定义的 SSA 值。
    pub value: SsaValueId,
    /// 静态类型。
    pub ty: ExecutionValueType,
}

impl BasicBlock {
    /// 立即返回无值的空块（占位 / 冻结）。
    pub fn empty_return(id: BlockId) -> Self {
        Self { id, parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::Return { values: Vec::new() } }
    }

    /// 返回单个既有 SSA 值的块。
    pub fn return_value(id: BlockId, value: SsaValueId) -> Self {
        Self { id, parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::return_value(value) }
    }
}
