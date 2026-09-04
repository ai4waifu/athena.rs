//! 中性请求边界合同测试（Living `26`）。

use athena_engine::{
    api::{AthenaEngine, AthenaRequest, ControlPlan, DefinitionEvaluationTiming, LoweringOutcome, SessionCommand},
    runtime::{CoverageStatus, Session},
};
use athena_types::{Diagnostic, DiagnosticCode, SymbolId};

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
    assert_eq!(session.results.count(), 1);
}

#[test]
fn execute_request_command_is_explicit_unsupported() {
    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let value = session.builder().int(1, Default::default());
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol: SymbolId(0),
        value,
        timing: DefinitionEvaluationTiming::Immediate,
    });
    assert_eq!(request.kind_name(), "Command");
    let err = engine.execute_request(&mut session, request).expect_err("command unsupported");
    assert_eq!(err.code, DiagnosticCode::UnsupportedOperation);
    assert_eq!(session.results.count(), 1);
    let stored = session.results.get(athena_types::ResultId(0)).expect("stored");
    assert_eq!(stored.coverage, CoverageStatus::Unsupported);
}

#[test]
fn execute_request_control_is_explicit_unsupported() {
    let engine = AthenaEngine::new();
    let mut session = Session::new();
    let body = AthenaRequest::Term(session.builder().int(0, Default::default()));
    let request = AthenaRequest::Control(ControlPlan::LexicalScope { body: Box::new(body) });
    assert_eq!(request.kind_name(), "Control");
    let err = engine.execute_request(&mut session, request).expect_err("control unsupported");
    assert_eq!(err.code, DiagnosticCode::UnsupportedOperation);
    assert_eq!(session.results.count(), 1);
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
