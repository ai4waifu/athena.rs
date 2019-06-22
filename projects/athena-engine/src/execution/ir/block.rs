//! Basic blocks with explicit block arguments (no implicit stack parameters).

use super::ids::{BlockId, SsaValueId};
use super::operation::Operation;
use super::terminator::Terminator;
use super::types::ExecutionValueType;

/// One SSA basic block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicBlock {
    /// Block identity.
    pub id: BlockId,
    /// Formal parameters (block arguments).
    pub parameters: Vec<BlockParameter>,
    /// Ordered SSA operations.
    pub operations: Vec<Operation>,
    /// Unique terminator.
    pub terminator: Terminator,
}

/// Typed block argument.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockParameter {
    /// SSA value defined at block entry.
    pub value: SsaValueId,
    /// Static type.
    pub ty: ExecutionValueType,
}

impl BasicBlock {
    /// Empty block that immediately returns no values (placeholder / freeze).
    pub fn empty_return(id: BlockId) -> Self {
        Self {
            id,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::Return { values: Vec::new() },
        }
    }

    /// Block that returns a single preexisting SSA value.
    pub fn return_value(id: BlockId, value: SsaValueId) -> Self {
        Self {
            id,
            parameters: Vec::new(),
            operations: Vec::new(),
            terminator: Terminator::return_value(value),
        }
    }
}
