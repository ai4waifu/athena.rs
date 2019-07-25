//! Backend contract — native JIT, WASM, and domain kernels consume the same `ExecutionIR`.

use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId};

use crate::{
    execution::{ir::ExecutionModule, reference::ReferenceExecutor},
    runtime::session::Session,
};

/// Capability / ABI fingerprint used in code-cache keys (not `TermId` indices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BackendAbiFingerprint(pub u64);

impl BackendAbiFingerprint {
    /// Derive a cache key from a verified module fingerprint and backend kind.
    pub fn of_module(module: &ExecutionModule, kind: BackendKind) -> Self {
        use std::{
            collections::hash_map::DefaultHasher,
            hash::{Hash, Hasher},
        };
        let mut hasher = DefaultHasher::new();
        0x4245_4142_4946_5052u64.hash(&mut hasher); // "BEABIFPR"
        module.fingerprint.0.hash(&mut hasher);
        core::mem::discriminant(&kind).hash(&mut hasher);
        Self(hasher.finish())
    }
}

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

impl ExecutionBackend for ReferenceExecutor {
    fn kind(&self) -> BackendKind {
        BackendKind::Reference
    }

    fn execute(&self, session: &mut Session, module: &ExecutionModule) -> Result<ResultId> {
        ReferenceExecutor::execute(self, session, module, None)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::request::AthenaRequest, execution::compiler::ExecutionCompiler};

    #[test]
    fn unwired_backends_return_typed_diagnostic() {
        let mut session = Session::new();
        let module = ExecutionModule::empty();
        let backends: [(Box<dyn ExecutionBackend>, &str); 3] = [
            (Box::new(NativeJitBackend::default()), "NativeJitBackend"),
            (Box::new(WasmBackend::default()), "WasmBackend"),
            (Box::new(DomainKernelBackend::default()), "DomainKernelBackend"),
        ];
        for (backend, component) in backends {
            let err = backend.execute(&mut session, &module).expect_err(component);
            assert_eq!(err.details.get("component").map(|v| v.to_string()).as_deref(), Some(component));
            assert_eq!(err.details.get("status").map(|v| v.to_string()).as_deref(), Some("contract_frozen_not_wired"));
        }
    }

    #[test]
    fn reference_backend_replays_term_module() {
        let mut session = Session::new();
        let term = session.builder().int(11, Default::default());
        let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
        let result_id = ExecutionBackend::execute(&ReferenceExecutor::new(), &mut session, &module).expect("ref");
        let loaded = session.results.get(result_id).expect("result");
        assert_eq!(loaded.symbolic_term, Some(term));
        let abi = BackendAbiFingerprint::of_module(&module, BackendKind::Reference);
        let abi_jit = BackendAbiFingerprint::of_module(&module, BackendKind::NativeJit);
        assert_ne!(abi, abi_jit);
    }
}
