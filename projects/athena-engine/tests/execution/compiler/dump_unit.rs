//! Living `04` 编译阶段 dump / fingerprint / verifier 观测测试。

use athena_engine::{
    Session,
    api::request::AthenaRequest,
    execution::compiler::{ExecutionCompiler, PlanIntent, canonicalize_request, observe_compile, plan_from_request},
};

#[test]
fn compile_staged_builds_request_plan_before_module() {
    let mut session = Session::new();
    let term = session.builder().int(3, Default::default());
    let request = AthenaRequest::Term(term);
    let request_prog = canonicalize_request(&request);
    let plan_prog = plan_from_request(&request_prog);
    assert_eq!(request_prog.kind, "Term");
    assert_eq!(plan_prog.intent, PlanIntent::EvaluateTerm);
    assert_eq!(plan_prog.request_fingerprint, request_prog.fingerprint);

    let staged = ExecutionCompiler::new().compile_staged(&mut session, &request).expect("staged");
    assert_eq!(staged.request.fingerprint, request_prog.fingerprint);
    assert_eq!(staged.plan.fingerprint, plan_prog.fingerprint);
    assert_eq!(staged.cfg_ssa.module_fingerprint, staged.module.fingerprint);
}

#[test]
fn compile_observed_atom_term_stages() {
    let mut session = Session::new();
    let term = session.builder().int(3, Default::default());
    let request = AthenaRequest::Term(term);
    let (module, observation) = ExecutionCompiler::new().compile_observed(&mut session, &request).expect("observed");

    assert_eq!(observation.request.kind, "Term");
    assert_eq!(observation.request.term_index, Some(term.0));
    assert_eq!(observation.plan.intent, PlanIntent::EvaluateTerm);
    assert!(!observation.plan.provider_required);
    assert_eq!(observation.plan.request_fingerprint, observation.request.fingerprint);
    assert!(!observation.semantic.operations.is_empty());
    assert_eq!(observation.cfg_ssa.module_fingerprint, module.fingerprint);
    assert!(observation.cfg_ssa.text.contains("region 0"));
    assert!(observation.cfg_ssa.text.contains("LoadTerm"));

    let rendered = observation.render();
    assert!(rendered.contains("stage request"));
    assert!(rendered.contains("stage plan"));
    assert!(rendered.contains("stage semantic"));
    assert!(rendered.contains("stage cfg_ssa"));

    let again = observe_compile(&request, &module).expect("reobserve");
    assert_eq!(again.request.fingerprint, observation.request.fingerprint);
    assert_eq!(again.plan.fingerprint, observation.plan.fingerprint);
    assert_eq!(again.semantic.fingerprint, observation.semantic.fingerprint);
    assert_eq!(again.cfg_ssa.fingerprint, observation.cfg_ssa.fingerprint);
}

#[test]
fn compile_observed_boolean_constant_cfg_text() {
    let mut session = Session::new();
    let term = session.builder().boolean(true, Default::default());
    let request = AthenaRequest::Term(term);
    let (_module, observation) = ExecutionCompiler::new().compile_observed(&mut session, &request).expect("observed");
    assert!(observation.cfg_ssa.block_count >= 1);
    assert!(observation.cfg_ssa.text.contains("return %"));
}
