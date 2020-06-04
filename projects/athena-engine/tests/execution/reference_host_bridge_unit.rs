//! Reference → `ExecutionHost` 委托桥单元测试。

use athena_engine::{
    Session,
    execution::reference::ReferenceExecutor,
};
use athena_engine::api::request::AthenaRequest;
use athena_engine::execution::compiler::ExecutionCompiler;
use athena_ir::{ApplicationHead, SemanticOperator};

#[test]
fn reference_boolean_and_delegates_via_host_path() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let f = session.builder().boolean(false, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::And),
        vec![t, f],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let result_id = ReferenceExecutor::new()
        .execute(&mut session, &module, None)
        .expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(false))) => {}
        other => panic!("expected False, got {other:?}"),
    }
}

#[test]
fn reference_numeric_and_still_uses_truthiness_path() {
    let mut session = Session::new();
    let zero = session.builder().int(0, Default::default());
    let one = session.builder().int(1, Default::default());
    let term = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::And),
        vec![zero, one],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(term))
        .expect("compile");
    let result_id = ReferenceExecutor::new()
        .execute(&mut session, &module, None)
        .expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(false))) => {}
        other => panic!("expected And[0,1] == False, got {other:?}"),
    }
}
