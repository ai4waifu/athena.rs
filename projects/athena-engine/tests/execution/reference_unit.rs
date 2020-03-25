//! 自 `src/execution/reference/mod.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    api::request::AthenaRequest,
    execution::{
        compiler::ExecutionCompiler,
        ir::{
            BasicBlock, BlockEdge, BlockId, ConstantId, ConstantValue, ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation,
            OperationKind, Region, RegionId, SsaValueId, Terminator,
        },
        number_of,
        reference::*,
    },
    runtime::CoverageStatus,
};
use athena_ir::SemanticOperator;
use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, IndexSpec, IntegerIndex, Result, ResultId, SymbolId, TermId};
use std::{cmp::Ordering, collections::HashMap};

#[test]
fn execute_compiled_atom_term() {
    let mut session = Session::new();
    let term = session.builder().int(9, Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(term));
    assert_eq!(loaded.status, ComputationStatus::Exact);
    assert_eq!(loaded.coverage, CoverageStatus::Full);
}

#[test]
fn execute_boolean_branch() {
    let cond = SsaValueId(0);
    let then_v = SsaValueId(1);
    let else_v = SsaValueId(2);
    let entry = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(cond),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(0) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(BlockId(1)), else_edge: BlockEdge::jump(BlockId(2)) },
    };
    let then_block = BasicBlock {
        id: BlockId(1),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(then_v),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(1) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(then_v),
    };
    let else_block = BasicBlock {
        id: BlockId(2),
        parameters: Vec::new(),
        operations: vec![Operation {
            result: Some(else_v),
            result_type: ExecutionValueType::Boolean,
            kind: OperationKind::Constant { constant: ConstantId(2) },
            effect_in: None,
            effect_out: None,
        }],
        terminator: Terminator::return_value(else_v),
    };
    let region = Region {
        id: RegionId(0),
        entry: BlockId(0),
        blocks: vec![entry, then_block, else_block],
        result_types: vec![ExecutionValueType::Boolean],
    };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: vec![ConstantValue::boolean(true), ConstantValue::boolean(true), ConstantValue::boolean(false)],
        captured_roots: Vec::new(),
        regions: vec![region],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);

    let mut session = Session::new();
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("branch");
    let loaded = session.results.get(result_id).expect("result");
    let term = loaded.symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(true))) => {}
        other => panic!("expected true boolean term, got {other:?}"),
    }
}

#[test]
fn truthy_and_or_with_zero_one() {
    let mut session = Session::new();
    let and = athena_ir::ApplicationHead::Semantic(athena_ir::SemanticOperator::And);
    let or = athena_ir::ApplicationHead::Semantic(athena_ir::SemanticOperator::Or);
    let z = session.builder().int(0, Default::default());
    let one = session.builder().int(1, Default::default());
    let and_term = session.builder().application(and, vec![z, one], Default::default());
    let or_term = session.builder().application(or, vec![z, one], Default::default());

    let and_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(and_term)).expect("and");
    let and_id = ReferenceExecutor::new().execute(&mut session, &and_mod, None).expect("and exec");
    let and_out = session.results.get(and_id).expect("and result").symbolic_term.expect("term");
    match session.arena.get(and_out) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(false))) => {}
        other => panic!("expected And[0,1] == False, got {other:?}"),
    }

    let or_mod = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(or_term)).expect("or");
    let or_id = ReferenceExecutor::new().execute(&mut session, &or_mod, None).expect("or exec");
    let or_out = session.results.get(or_id).expect("or result").symbolic_term.expect("term");
    match session.arena.get(or_out) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Boolean(true))) => {}
        other => panic!("expected Or[0,1] == True, got {other:?}"),
    }
}

#[test]
fn unknown_head_marks_partial_unknown() {
    let mut session = Session::new();
    let foo_id = session.extensions.intern("FooBar");
    let foo = athena_ir::ApplicationHead::Extension(foo_id);
    let one = session.builder().int(1, Default::default());
    let term = session.builder().application(foo, vec![one], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("compile");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Unknown);
    assert_eq!(loaded.coverage, CoverageStatus::Partial);
    assert!(loaded.diagnostics.is_empty());
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(athena_ir::TermNode::Application { head: athena_ir::ApplicationHead::Extension(id), .. })
            if session.extensions.display_name(*id) == Some("FooBar") => {}
        other => panic!("expected residual FooBar[...], got {other:?}"),
    }
}

fn index_module(target: TermId, axes: Vec<athena_types::IndexSpec>) -> athena_engine::execution::ir::ExecutionModule {
    use athena_engine::execution::ir::{
        BasicBlock, BlockId, CapturedRoot, CapturedRootId, ExecutionModule, ExecutionValueType, ModuleFingerprint, Operation, OperationKind,
        Region, RegionId, SsaValueId, Terminator, verify_module,
    };
    let load = SsaValueId(0);
    let indexed = SsaValueId(1);
    let published = SsaValueId(2);
    let block = BasicBlock {
        id: BlockId(0),
        parameters: Vec::new(),
        operations: vec![
            Operation {
                result: Some(load),
                result_type: ExecutionValueType::Term,
                kind: OperationKind::LoadTerm { root: CapturedRootId(0) },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(indexed),
                result_type: ExecutionValueType::Term,
                kind: OperationKind::Index { target: load, axes },
                effect_in: None,
                effect_out: None,
            },
            Operation {
                result: Some(published),
                result_type: ExecutionValueType::Result,
                kind: OperationKind::PublishResult { source: indexed },
                effect_in: None,
                effect_out: None,
            },
        ],
        terminator: Terminator::return_value(published),
    };
    let mut module = ExecutionModule {
        inputs: Vec::new(),
        constants: Vec::new(),
        captured_roots: vec![CapturedRoot::term(target)],
        regions: vec![Region { id: RegionId(0), entry: BlockId(0), blocks: vec![block], result_types: vec![ExecutionValueType::Term] }],
        effect_edges: Vec::new(),
        exits: Vec::new(),
        provider_calls: Vec::new(),
        fingerprint: ModuleFingerprint(0),
    };
    module.fingerprint = ModuleFingerprint::of_module(&module);
    verify_module(&module).expect("verify");
    module
}

#[test]
fn index_oob_marks_invalid_index() {
    use athena_types::{IndexSpec, IntegerIndex};
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let list = session.builder().list(vec![a, b], Default::default());
    let module = index_module(list, vec![IndexSpec::Scalar(IntegerIndex(9))]);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.status, ComputationStatus::Invalid);
    assert_eq!(loaded.diagnostics[0].code, DiagnosticCode::InvalidIndex);
}

#[test]
fn index_range_extracts_slice() {
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let list = session.builder().list(vec![a, b, c], Default::default());
    let module = index_module(list, vec![IndexSpec::Range { start: IntegerIndex(1), end: IntegerIndex(2), step: 1 }]);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            assert_eq!(number_of(&session, items[0]).and_then(|n| n.as_exact_integer()), Some(1));
            assert_eq!(number_of(&session, items[1]).and_then(|n| n.as_exact_integer()), Some(2));
        }
        other => panic!("expected OrderedCollection[1, 2], got {other:?}"),
    }
}

#[test]
fn index_all_then_scalar_selects_column() {
    let mut session = Session::new();
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let d = session.builder().int(4, Default::default());
    let row0 = session.builder().list(vec![a, b], Default::default());
    let row1 = session.builder().list(vec![c, d], Default::default());
    let matrix = session.builder().list(vec![row0, row1], Default::default());
    let module = index_module(matrix, vec![IndexSpec::All, IndexSpec::Scalar(IntegerIndex(2))]);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module, None).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) if items.len() == 2 => {
            assert_eq!(number_of(&session, items[0]).and_then(|n| n.as_exact_integer()), Some(2));
            assert_eq!(number_of(&session, items[1]).and_then(|n| n.as_exact_integer()), Some(4));
        }
        other => panic!("expected OrderedCollection[2, 4], got {other:?}"),
    }
}
