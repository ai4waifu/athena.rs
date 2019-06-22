//! `ExecutionCompiler` — `AthenaRequest` + Session snapshot → [`ExecutionModule`](crate::execution::ir::ExecutionModule).
//!
//! Contract freeze only: no lowering implementation and no bridge to the old
//! stack VM / `KernelIR` path.

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::api::request::AthenaRequest;
use crate::execution::ir::ExecutionModule;
use crate::runtime::session::Session;

/// Compiles one request into a verified [`ExecutionModule`].
#[derive(Debug, Default)]
pub struct ExecutionCompiler {}

impl ExecutionCompiler {
    /// Create a compiler instance.
    pub fn new() -> Self {
        Self {}
    }

    /// Lower a request against a Session snapshot into `ExecutionIR`.
    ///
    /// Freeze status: returns [`DiagnosticCode::UnsupportedOperation`] until the
    /// cutover wires real lowering. Must not call the old VM.
    pub fn compile(&self, _session: &Session, _request: &AthenaRequest) -> Result<ExecutionModule> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "ExecutionCompiler")
            .detail("status", "contract_frozen_not_wired"))
    }
}
