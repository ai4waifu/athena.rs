//! Declared guard / failure / deoptimization exits.

use super::ids::{BlockId, ExitId};
use super::types::ExecutionValueType;

/// Why an exit edge may be taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitKind {
    /// Guard predicate failed.
    GuardRejected,
    /// Required capability missing.
    CapabilityMissing,
    /// Budget exhausted → partial result path.
    BudgetExhausted,
    /// Cancellation requested.
    Cancelled,
    /// Explicit deoptimization to a declared runtime exit (never an old VM).
    Deoptimize,
    /// Provider returned typed unsupported / unknown.
    ProviderDiagnostic,
}

/// One declared module exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredExit {
    /// Exit table index.
    pub id: ExitId,
    /// Classification.
    pub kind: ExitKind,
    /// Optional continuation block inside the module.
    pub continuation: Option<BlockId>,
    /// Values expected on the exit edge.
    pub result_types: Vec<ExecutionValueType>,
}

impl DeclaredExit {
    /// Guard rejection without in-module continuation.
    pub fn guard_reject(id: ExitId) -> Self {
        Self {
            id,
            kind: ExitKind::GuardRejected,
            continuation: None,
            result_types: Vec::new(),
        }
    }
}
