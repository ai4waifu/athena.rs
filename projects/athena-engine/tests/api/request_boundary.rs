//! 中性请求边界合同测试（Living `26`）。

use athena_engine::{
    api::{AthenaEngine, AthenaRequest, ControlPlan, LoweringOutcome, SessionCommand},
    runtime::{CoverageStatus, Session},
};
use athena_types::{BindingEvaluationPolicy, BindingKind, Diagnostic, DiagnosticCode};

#[test]
fn lowering_outcome_accepted_term_kind() {
    let mut session = Session::new();
    let term = session.builder().int(1, Default::default());
    let outcome = LoweringOutcome::accepted(AthenaRequest::Term(term));
    assert!(outcome.is_accepted());
    match outcome {
        LoweringOutcome::Accepted(AthenaRequest::Term(id)) => assert_eq!(id, term),
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn execute_request_term_records_computation_result() {
    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let term = session.builder().int(7, Default::default());
    let result_id = engine.execute_request(&mut session, AthenaRequest::Term(term)).expect("term");
    let loaded = session.results.get(result_id).expect("payload");
    assert!(loaded.symbolic_term.is_some());
    assert!(loaded.value.is_some());
    assert_eq!(loaded.coverage, CoverageStatus::Full);
    assert!(loaded.provenance.is_some());
    assert_eq!(session.results.count(), 1);
}

#[test]
fn execute_request_domain_goal_preserves_domain_payload() {
    use athena_engine::{
        domains::{
            dispatch::DomainRequest,
            number_theory::{NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue},
        },
        runtime::{ResultProviderId, RuntimeValue},
    };
    use athena_numeric::Integer;

    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let request = AthenaRequest::Goal(athena_engine::api::DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
        a: Integer::from_i64(12),
        b: Integer::from_i64(8),
    })));
    let result_id = engine.execute_request(&mut session, request).expect("goal");
    let loaded = session.results.get(result_id).expect("payload");
    assert_eq!(loaded.status, athena_types::ComputationStatus::Exact);
    assert_eq!(loaded.coverage, CoverageStatus::Full);
    assert_eq!(loaded.provider, Some(ResultProviderId::NUMBER_THEORY.stamped()));
    let value_id = loaded.value.expect("value");
    match session.values.get(value_id).expect("runtime") {
        RuntimeValue::Domain(athena_engine::domains::dispatch::DomainResult::NumberTheory(NumberTheoryResult::Exact {
            value: NumberTheoryValue::Integer(n),
        })) => assert_eq!(n, &Integer::from_i64(4)),
        other => panic!("expected preserved domain gcd payload, got {other:?}"),
    }
}

#[test]
fn execute_request_command_define_via_execution_ir() {
    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let sym = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(sym) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let value = session.builder().int(1, Default::default());
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    assert_eq!(request.kind_name(), "Command");
    let result_id = engine.execute_request(&mut session, request).expect("define");
    let stored = session.results.get(result_id).expect("stored");
    assert_eq!(stored.coverage, CoverageStatus::Full);
    assert_eq!(session.defs.binding(symbol), Some(value));
}

#[test]
fn execute_request_control_lexical_scope_via_execution_ir() {
    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let body_term = session.builder().int(0, Default::default());
    let body = AthenaRequest::Term(body_term);
    let request = AthenaRequest::Control(ControlPlan::LexicalScope { body: Box::new(body) });
    assert_eq!(request.kind_name(), "Control");
    let result_id = engine.execute_request(&mut session, request).expect("scope");
    let stored = session.results.get(result_id).expect("stored");
    assert_eq!(stored.symbolic_term, Some(body_term));
    assert_eq!(stored.coverage, CoverageStatus::Full);
}

#[test]
fn control_plan_covers_neutral_loop_and_recover_shapes() {
    let mut session = Session::new();
    let zero = session.builder().int(0, Default::default());
    let one = session.builder().int(1, Default::default());
    let _ = ControlPlan::LoopWhile { condition: zero, body: Box::new(AthenaRequest::Term(zero)) };
    let _ = ControlPlan::CountedLoop { variable: zero, iterator: one, body: Box::new(AthenaRequest::Term(one)) };
    let _ = ControlPlan::Recover { body: Box::new(AthenaRequest::Term(zero)), handler: Box::new(AthenaRequest::Term(one)) };
    let _ = ControlPlan::Reject;
    let _ = ControlPlan::Cond { arms: vec![(zero, Box::new(AthenaRequest::Term(one)))], otherwise: Some(Box::new(AthenaRequest::Term(zero))) };
    let _ = ControlPlan::LocalScope { body: Box::new(AthenaRequest::Term(zero)) };
}

#[test]
fn rejected_lowering_does_not_execute() {
    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let diagnostic = Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "dialect_gap");
    let err = engine.execute_lowering_outcome(&mut session, LoweringOutcome::rejected(diagnostic)).expect_err("rejected");
    assert_eq!(err.code, DiagnosticCode::UnsupportedOperation);
    assert_eq!(session.results.count(), 0);
}
