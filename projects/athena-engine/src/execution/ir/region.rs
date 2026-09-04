//! Regions group basic blocks that share a control-flow graph.

use super::block::BasicBlock;
use super::ids::{BlockId, RegionId};
use super::types::ExecutionValueType;

/// One region inside an [`super::ExecutionModule`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Region identity.
    pub id: RegionId,
    /// Entry block.
    pub entry: BlockId,
    /// Blocks owned by this region.
    pub blocks: Vec<BasicBlock>,
    /// Values returned by successful region completion.
    pub result_types: Vec<ExecutionValueType>,
}

impl Region {
    /// Single-block region.
    pub fn from_entry_block(id: RegionId, block: BasicBlock, result_types: Vec<ExecutionValueType>) -> Self {
        let entry = block.id;
        Self { id, entry, blocks: vec![block], result_types }
    }
}
