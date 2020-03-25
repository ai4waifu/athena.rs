//! 自 `src/execution/mod.rs` 迁出的原内联测试。

use athena_engine::{Session, api::request::AthenaRequest, execution::execute_ir_request};

#[test]
fn execute_ir_request_atom_term() {
    let mut session = Session::new();
    let term = session.builder().int(4, Default::default());
    let result_id = execute_ir_request(&mut session, AthenaRequest::Term(term)).expect("ir");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(term));
}
