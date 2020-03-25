//! Region 将共享控制流图的基本块归为一组。

use super::{
    block::BasicBlock,
    ids::{BlockId, RegionId},
    types::ExecutionValueType,
};

/// [`super::ExecutionModule`] 内的一个 region。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Region 标识。
    pub id: RegionId,
    /// 入口基本块。
    pub entry: BlockId,
    /// 本 region 拥有的基本块。
    pub blocks: Vec<BasicBlock>,
    /// Region 成功完成时返回的值类型。
    pub result_types: Vec<ExecutionValueType>,
}

impl Region {
    /// 单基本块 region。
    pub fn from_entry_block(id: RegionId, block: BasicBlock, result_types: Vec<ExecutionValueType>) -> Self {
        let entry = block.id;
        Self { id, entry, blocks: vec![block], result_types }
    }
}
