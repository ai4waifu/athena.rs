//! Explicit block terminators (no implicit instruction pointer).

use super::ids::{BlockId, ExitId, SsaValueId};

/// Successor edge with block arguments (SSA phi via block args).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockEdge {
    /// Destination block.
    pub target: BlockId,
    /// Arguments passed into the destination block parameters.
    pub arguments: Vec<SsaValueId>,
}

/// Closed terminator set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Terminator {
    /// Conditional branch on a typed Boolean SSA value.
    Branch {
        /// Predicate.
        condition: SsaValueId,
        /// Taken when predicate is true.
        then_edge: BlockEdge,
        /// Taken when predicate is false.
        else_edge: BlockEdge,
    },
    /// Multi-way branch on a discrete discriminant.
    Switch {
        /// Discriminant SSA value.
        discriminant: SsaValueId,
        /// `(case value index → edge)` table (exact encoding filled by compiler).
        cases: Vec<(u32, BlockEdge)>,
        /// Default edge when no case matches.
        default: BlockEdge,
    },
    /// Successful return from the current region / module.
    Return {
        /// Returned SSA values (order fixed by region signature).
        values: Vec<SsaValueId>,
    },
    /// Hard reject with a declared exit / diagnostic path.
    Reject {
        /// Optional module exit descriptor.
        exit: Option<ExitId>,
    },
    /// Yield control back to runtime / provider (not a scheduler).
    Yield {
        /// Values handed to the runtime context.
        values: Vec<SsaValueId>,
        /// Resume edge after the yield completes.
        resume: BlockEdge,
    },
    /// Unreachable marker for verifier completeness.
    Unreachable,
}

impl BlockEdge {
    /// Edge with no block arguments.
    pub fn jump(target: BlockId) -> Self {
        Self { target, arguments: Vec::new() }
    }
}

impl Terminator {
    /// Simple return of one value.
    pub fn return_value(value: SsaValueId) -> Self {
        Self::Return { values: vec![value] }
    }
}
