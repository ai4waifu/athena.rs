//! 生产路径经 verified CFG 子集走 `athena-vm`（含 `LoadTerm` 原子项）。

use athena_engine::{
    Session,
    api::request::AthenaRequest,
    execution::execute_ir_request,
    runtime::CoverageStatus,
};
use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::ComputationStatus;

#[test]
fn execute_ir_request_not_uses_vm_path() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let term = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Not), vec![t], Default::default());
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.coverage, CoverageStatus::Full);
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Boolean(false))) => {}
        other => panic!("expected false, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_atom_term_uses_vm_load_term() {
    let mut session = Session::new();
    let term = session.builder().int(7, Default::default());
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.coverage, CoverageStatus::Full);
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    assert_eq!(loaded.symbolic_term, Some(term));
    match session.arena.get(term) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(7) => {}
        other => panic!("expected int 7, got {other:?}"),
    }
}

#[test]
fn execute_ir_request_hold_atom_uses_vm_load_term() {
    let mut session = Session::new();
    let inner = session.builder().int(5, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Hold),
        vec![inner],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    assert_eq!(loaded.symbolic_term, Some(term));
}

#[test]
fn execute_ir_request_add_integers_uses_vm_host() {
    let mut session = Session::new();
    let a = session.builder().int(2, Default::default());
    let b = session.builder().int(3, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::Add),
        vec![a, b],
        Default::default(),
    );
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.provenance.as_ref().map(|p| p.request_kind), Some("ExecutionIR/athena-vm"));
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5) => {}
        other => panic!("expected 5, got {other:?}"),
    }
}
