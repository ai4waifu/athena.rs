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
fn compile_routes_root_by_plan_intent() {
    let mut session = Session::new();
    let term = session.builder().int(7, Default::default());
    let request = AthenaRequest::Term(term);
    let request_prog = canonicalize_request(&request);
    assert_eq!(request_prog.term_index, Some(term.0));
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("compile consumes request and plan");
    assert!(!module.regions.is_empty());

    let staged = ExecutionCompiler::new().compile_staged(&mut session, &request).expect("staged");
    assert_eq!(staged.plan.intent, PlanIntent::EvaluateTerm);
    assert_eq!(staged.plan.request_fingerprint, request_prog.fingerprint);
    assert!(!staged.plan.provider_required);
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
fn compile_goal_consumes_domain_payload_from_request_program() {
    use athena_engine::domains::linear_algebra::{LinearAlgebraRequest, MatrixValue};
    use athena_engine::api::request::DomainGoal;
    use athena_engine::domains::DomainRequest;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let mat = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![Integer::from_i64(1), Integer::from_i64(2), Integer::from_i64(3), Integer::from_i64(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Transpose {
        matrix: mat,
    })));
    let staged = ExecutionCompiler::new().compile_staged(&mut session, &request).expect("goal staged");
    assert_eq!(staged.request.kind, "Goal");
    assert_eq!(staged.request.payload_tag, Some("LinearAlgebra"));
    assert!(staged.request.domain_payload.is_some());
    assert_eq!(staged.plan.intent, PlanIntent::DomainProvider);
    assert!(staged.plan.provider_required);
}

#[test]
fn compile_command_consumes_owned_session_command() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let symbol = session.arena.symbols_mut().intern("x");
    let value = session.builder().int(1, Default::default());
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let staged = ExecutionCompiler::new().compile_staged(&mut session, &request).expect("command staged");
    assert_eq!(staged.request.payload_tag, Some("Define"));
    assert!(staged.request.command.is_some());
    assert_eq!(staged.plan.intent, PlanIntent::SessionCommand);
}

#[test]
fn compile_control_consumes_owned_control_plan() {
    use athena_engine::api::request::ControlPlan;

    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![AthenaRequest::Term(one), AthenaRequest::Term(two)],
    });
    let staged = ExecutionCompiler::new().compile_staged(&mut session, &request).expect("control staged");
    assert_eq!(staged.request.payload_tag, Some("Sequence"));
    assert!(staged.request.control.is_some());
    assert_eq!(staged.plan.intent, PlanIntent::RunControl);
}

#[test]
fn nested_goal_inside_sequence_uses_prepared_domain_payload() {
    use athena_engine::api::request::{ControlPlan, DomainGoal};
    use athena_engine::domains::linear_algebra::{LinearAlgebraRequest, MatrixValue};
    use athena_engine::domains::DomainRequest;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let mat = session.matrix_objects.intern(
        MatrixValue::from_integers_row_major(2, 2, vec![Integer::from_i64(1), Integer::from_i64(2), Integer::from_i64(3), Integer::from_i64(4)])
            .unwrap(),
    );
    let goal = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Transpose {
        matrix: mat,
    })));
    let one = session.builder().int(1, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![goal, AthenaRequest::Term(one)],
    });
    let staged = ExecutionCompiler::new().compile_staged(&mut session, &request).expect("nested goal");
    assert_eq!(staged.plan.intent, PlanIntent::RunControl);
    assert!(staged.cfg_ssa.text.contains("CallProvider"));
    assert!(session.domain_payloads.len() >= 1);
}
