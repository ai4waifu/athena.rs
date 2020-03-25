//! 自 `src/execution/backend/mod.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    api::request::AthenaRequest,
    execution::{backend::*, compiler::ExecutionCompiler, ir::ExecutionModule, reference::ReferenceExecutor},
};
use athena_types::{Diagnostic, DiagnosticCode, Result, ResultId};

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
