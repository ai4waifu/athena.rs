//! 生产路径优先走线性 Boolean VM 降级。

use athena_engine::{
    Session,
    api::request::AthenaRequest,
    execution::execute_ir_request,
    runtime::CoverageStatus,
};
use athena_ir::{ApplicationHead, SemanticOperator};
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
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(false))) => {}
        other => panic!("expected false, got {other:?}"),
    }
}
