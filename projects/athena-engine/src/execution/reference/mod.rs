//! `ReferenceExecutor` — correctness / replay backend for [`ExecutionModule`](crate::execution::ir::ExecutionModule).
//!
//! Executes SSA blocks without an operand stack. Not a wrapper around the old VM.

use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId};

use crate::execution::ir::ExecutionModule;
use crate::runtime::session::Session;

/// Semantic oracle backend shared by parity tests and deterministic replay.
#[derive(Debug, Default)]
pub struct ReferenceExecutor {}

impl ReferenceExecutor {
    /// Create a reference executor.
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a verified module in the given Session / runtime context.
    ///
    /// Freeze status: rejects until cutover implements block-wise SSA evaluation.
    pub fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "ReferenceExecutor")
            .detail("status", "contract_frozen_not_wired"))
    }
}
