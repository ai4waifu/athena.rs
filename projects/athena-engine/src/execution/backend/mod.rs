//! Backend contract — native JIT, WASM, and domain kernels consume the same `ExecutionIR`.

use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId};

use crate::execution::ir::ExecutionModule;
use crate::runtime::session::Session;

/// Capability / ABI fingerprint used in code-cache keys (not `TermId` indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendAbiFingerprint(pub u64);

/// Which executable backend is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    /// Correctness / replay oracle.
    Reference,
    /// Optional native JIT.
    NativeJit,
    /// Optional WASM.
    Wasm,
    /// Provider-private pure-region kernel artifact.
    DomainKernel,
}

/// Shared entry contract for every backend.
pub trait ExecutionBackend {
    /// Backend classification.
    fn kind(&self) -> BackendKind;

    /// Execute a verified module. Unsupported paths must return a typed diagnostic
    /// — never silently fall back to another execution model.
    fn execute(&self, session: &mut Session, module: &ExecutionModule) -> Result<ResultId>;
}

/// Placeholder native JIT backend (not wired).
#[derive(Debug, Default)]
pub struct NativeJitBackend {}

impl ExecutionBackend for NativeJitBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::NativeJit
    }

    fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "NativeJitBackend")
            .detail("status", "contract_frozen_not_wired"))
    }
}

/// Placeholder WASM backend (not wired).
#[derive(Debug, Default)]
pub struct WasmBackend {}

impl ExecutionBackend for WasmBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Wasm
    }

    fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "WasmBackend")
            .detail("status", "contract_frozen_not_wired"))
    }
}

/// Placeholder domain-kernel backend (private artifacts only via `CallProvider`).
#[derive(Debug, Default)]
pub struct DomainKernelBackend {}

impl ExecutionBackend for DomainKernelBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::DomainKernel
    }

    fn execute(&self, _session: &mut Session, _module: &ExecutionModule) -> Result<ResultId> {
        Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("component", "DomainKernelBackend")
            .detail("status", "contract_frozen_not_wired"))
    }
}
