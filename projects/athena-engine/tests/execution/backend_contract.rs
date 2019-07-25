//! Backend contract: reference replay vs unwired JIT / WASM / domain kernel diagnostics.

use athena_engine::{
    api::request::AthenaRequest,
    execution::{
        backend::{BackendAbiFingerprint, BackendKind, DomainKernelBackend, ExecutionBackend, NativeJitBackend, WasmBackend},
        compiler::ExecutionCompiler,
        reference::ReferenceExecutor,
    },
    runtime::Session,
};
use athena_types::ComputationStatus;

#[test]
fn reference_backend_trait_matches_direct_execute() {
    let mut session = Session::new();
    let term = session.builder().int(4, Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");

    let via_trait = ExecutionBackend::execute(&ReferenceExecutor::new(), &mut session, &module).expect("trait");
    let via_direct = ReferenceExecutor::new().execute(&mut session, &module, None).expect("direct");

    let a = session.results.get(via_trait).expect("a");
    let b = session.results.get(via_direct).expect("b");
    assert_eq!(a.symbolic_term, Some(term));
    assert_eq!(b.symbolic_term, Some(term));
    assert_eq!(a.status, ComputationStatus::Exact);
    assert_eq!(b.status, ComputationStatus::Exact);
    assert_eq!(BackendKind::Reference, ReferenceExecutor::new().kind());
}

#[test]
fn unwired_backends_never_silently_fallback() {
    let mut session = Session::new();
    let term = session.builder().int(1, Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");

    let cases: [(&dyn ExecutionBackend, &str); 3] = [
        (&NativeJitBackend {}, "NativeJitBackend"),
        (&WasmBackend {}, "WasmBackend"),
        (&DomainKernelBackend {}, "DomainKernelBackend"),
    ];
    for (backend, component) in cases {
        let err = backend.execute(&mut session, &module).expect_err(component);
        assert_eq!(err.details.get("component").map(|v| v.to_string()).as_deref(), Some(component));
        assert_eq!(err.details.get("status").map(|v| v.to_string()).as_deref(), Some("contract_frozen_not_wired"));
        assert_ne!(BackendAbiFingerprint::of_module(&module, backend.kind()), BackendAbiFingerprint::of_module(&module, BackendKind::Reference));
    }
}
