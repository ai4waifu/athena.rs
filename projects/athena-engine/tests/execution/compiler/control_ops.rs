use athena_engine::{
    Session,
    api::request::{AthenaRequest, ControlPlan, SessionCommand},
    execution::{
        compiler::ExecutionCompiler,
        ir::{CapturedRoot, EffectKind, OperationKind},
        reference::ReferenceExecutor,
    },
};
use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::ComputationStatus;

#[test]
fn compile_and_execute_boolean_branch() {
    let mut session = Session::new();
    let cond = session.builder().boolean(true, Default::default());
    let then_term = session.builder().int(1, Default::default());
    let else_term = session.builder().int(0, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Branch {
        condition: cond,
        then_branch: Box::new(AthenaRequest::Term(then_term)),
        else_branch: Some(Box::new(AthenaRequest::Term(else_term))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("branch");
    assert_eq!(module.regions[0].blocks.len(), 3);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(then_term));
    assert_eq!(loaded.status, ComputationStatus::Exact);
}

#[test]
fn compile_and_execute_define_write_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let sym_term = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let value = session.builder().int(42, Default::default());
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define");
    assert!(!module.effect_edges.is_empty());
    ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.defs.binding(symbol), Some(value));
}

#[test]
fn compile_and_execute_define_matrix() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let sym_term = session.builder().symbol("A", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let matrix = MatrixValue::from_integers_row_major(1, 2, vec![Integer::from(1), Integer::from(2)]).expect("matrix");
    let matrix_ref = session.matrix_objects.intern(matrix);
    let request = AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix: matrix_ref });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define matrix");
    assert!(!module.effect_edges.is_empty());
    ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.matrix_binding(symbol), Some(matrix_ref));
    assert_eq!(session.defs.binding(symbol), None);
}

#[test]
fn compile_and_execute_index_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_numeric::Integer;
    use athena_types::{IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let symbol = match session.arena.get(a) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let matrix = MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(1), Integer::from(2), Integer::from(3), Integer::from(4)]).expect("matrix");
    let matrix_ref = session.matrix_objects.intern(matrix);
    let define = AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix: matrix_ref });
    let read = AthenaRequest::Control(ControlPlan::Index {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(1)), IndexSpec::Scalar(IntegerIndex(2))],
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence { steps: vec![define, read] });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("index matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected A(1,2) == 2, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_store_index_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_numeric::Integer;
    use athena_types::{IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let symbol = match session.arena.get(a) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let nine = session.builder().int(9, Default::default());
    let matrix = MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(1), Integer::from(2), Integer::from(3), Integer::from(4)]).expect("matrix");
    let matrix_ref = session.matrix_objects.intern(matrix);
    let define = AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix: matrix_ref });
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(1)), IndexSpec::Scalar(IntegerIndex(2))],
        value: nine,
    });
    let read = AthenaRequest::Control(ControlPlan::Index {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(1)), IndexSpec::Scalar(IntegerIndex(2))],
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, read],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("store matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(9) => {}
        other => panic!("expected A(1,2) == 9 after StoreIndex, got {other:?}"),
    }
    assert!(session.matrix_binding(symbol).is_some());
    assert!(session.defs.binding(symbol).is_none());
}

#[test]
fn compile_and_execute_store_index_grows_matrix_row_vector() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_numeric::Integer;
    use athena_types::{IndexSpec, IntegerIndex, IntegerOffset};

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let symbol = match session.arena.get(a) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let five = session.builder().int(5, Default::default());
    let matrix = MatrixValue::from_integers_row_major(1, 4, vec![
        Integer::from(1),
        Integer::from(2),
        Integer::from(3),
        Integer::from(4),
    ])
    .expect("row");
    let matrix_ref = session.matrix_objects.intern(matrix);
    let define = AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix: matrix_ref });
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: a,
        axes: vec![IndexSpec::EndRelative(IntegerOffset(1))],
        value: five,
    });
    let read = AthenaRequest::Term(a);
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, read],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("grow row");
    let _result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let grown = session.matrix_objects.resolve_owning(session.matrix_binding(symbol).expect("matrix own")).expect("value");
    assert_eq!(grown.shape().rows, 1);
    assert_eq!(grown.shape().cols, 5);
    assert_eq!(grown.get(0, 4).expect("cell").owning_copy(), athena_engine::domains::linear_algebra::MatrixEntry::Integer(Integer::from(5)));
}

#[test]
fn compile_and_execute_read_matrix_binding_as_nested_list() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let symbol = match session.arena.get(a) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let matrix = MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(1), Integer::from(2), Integer::from(3), Integer::from(4)]).expect("matrix");
    let matrix_ref = session.matrix_objects.intern(matrix);
    let define = AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix: matrix_ref });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, AthenaRequest::Term(a)],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("read matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Nested list {{1,2},{3,4}} shape for dialect render.
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: rows, .. }) if rows.len() == 2 => {}
        other => panic!("expected nested list matrix projection, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_accumulate_on_row_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::{MatrixEntry, MatrixValue};
    use athena_engine::runtime::RuntimeValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 3, vec![Integer::from(1), Integer::from(2), Integer::from(3)]).expect("matrix"));
    let acc = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Accumulate), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(acc),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("accumulate");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let value_id = session.results.get(result_id).expect("result").value.expect("value");
    let matrix_ref = match session.values.get(value_id) {
        Some(RuntimeValue::Matrix(m)) => *m,
        other => panic!("expected Matrix RuntimeValue, got {other:?}"),
    };
    let out = session.matrix_objects.resolve_owning(matrix_ref).expect("payload");
    assert_eq!(out.shape().rows, 1);
    assert_eq!(out.shape().cols, 3);
    // prefix sums 1, 3, 6
    let e0 = out.get(0, 0).expect("e0");
    let e1 = out.get(0, 1).expect("e1");
    let e2 = out.get(0, 2).expect("e2");
    assert!(matches!(e0, MatrixEntry::Rational(ref r) if r.is_integer() && r.numerator().to_i64() == Some(1)));
    assert!(matches!(e1, MatrixEntry::Rational(ref r) if r.is_integer() && r.numerator().to_i64() == Some(3)));
    assert!(matches!(e2, MatrixEntry::Rational(ref r) if r.is_integer() && r.numerator().to_i64() == Some(6)));
}

#[test]
fn compile_and_execute_differences_on_row_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::{MatrixEntry, MatrixValue};
    use athena_engine::runtime::RuntimeValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 3, vec![Integer::from(1), Integer::from(3), Integer::from(6)]).expect("matrix"));
    let diffs = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Differences), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(diffs),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("differences");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let value_id = session.results.get(result_id).expect("result").value.expect("value");
    let matrix_ref = match session.values.get(value_id) {
        Some(RuntimeValue::Matrix(m)) => *m,
        other => panic!("expected Matrix RuntimeValue, got {other:?}"),
    };
    let out = session.matrix_objects.resolve_owning(matrix_ref).expect("payload");
    assert_eq!(out.shape().rows, 1);
    assert_eq!(out.shape().cols, 2);
    let e0 = out.get(0, 0).expect("e0");
    let e1 = out.get(0, 1).expect("e1");
    assert!(matches!(e0, MatrixEntry::Rational(ref r) if r.is_integer() && r.numerator().to_i64() == Some(2)));
    assert!(matches!(e1, MatrixEntry::Rational(ref r) if r.is_integer() && r.numerator().to_i64() == Some(3)));
}

#[test]
fn compile_and_execute_sum_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("matrix"));
    let sum = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Sum), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(sum),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("sum");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Column sums {1+3, 2+4} = {4, 6}
    match session.arena.get(term) {
        Some(TermNode::Collection { elements, .. }) if elements.len() == 2 => {
            assert!(matches!(session.arena.get(elements[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4)));
            assert!(matches!(session.arena.get(elements[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(6)));
        }
        other => panic!("expected column-sum list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_product_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("matrix"));
    let product = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Product), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(product),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("product");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Column products {1*3, 2*4} = {3, 8}
    match session.arena.get(term) {
        Some(TermNode::Collection { elements, .. }) if elements.len() == 2 => {
            assert!(matches!(session.arena.get(elements[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
            assert!(matches!(session.arena.get(elements[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(8)));
        }
        other => panic!("expected column-product list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_product_on_row_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 3, vec![Integer::from(2), Integer::from(3), Integer::from(4)]).expect("matrix"));
    let product = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Product), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(product),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("product row");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(24)));
}

#[test]
fn compile_and_execute_length_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 3, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
            Integer::from(5),
            Integer::from(6),
        ]).expect("matrix"));
    let length = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Length), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(length),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("length");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
}

#[test]
fn compile_and_execute_diagonal_matrix_returns_matrix_value() {
    use athena_engine::runtime::RuntimeValue;
    use athena_ir::ApplicationHead;
    use athena_types::CollectionKind;

    let mut session = Session::new();
    let d0 = session.builder().int(1, Default::default());
    let d1 = session.builder().int(2, Default::default());
    let diag = session.builder().collection(CollectionKind::OrderedCollection, vec![d0, d1], Default::default());
    let call = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::DiagonalMatrix),
        vec![diag],
        Default::default(),
    );
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(call))
        .expect("diag");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let value_id = session.results.get(result_id).expect("result").value.expect("value");
    let matrix_ref = match session.values.get(value_id) {
        Some(RuntimeValue::Matrix(m)) => *m,
        other => panic!("expected Matrix RuntimeValue, got {other:?}"),
    };
    let matrix = session.matrix_objects.resolve_owning(matrix_ref).expect("payload");
    assert_eq!(matrix.shape().rows, 2);
    assert_eq!(matrix.shape().cols, 2);
}

#[test]
fn compile_and_execute_eye_returns_matrix_value() {
    use athena_ir::ApplicationHead;
    use athena_engine::runtime::RuntimeValue;

    let mut session = Session::new();
    let n = session.builder().int(2, Default::default());
    let eye = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Eye), vec![n], Default::default());
    let module = ExecutionCompiler::new()
        .compile(&mut session, &AthenaRequest::Term(eye))
        .expect("eye");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let value_id = session.results.get(result_id).expect("result").value.expect("value");
    let matrix_ref = match session.values.get(value_id) {
        Some(RuntimeValue::Matrix(m)) => *m,
        other => panic!("expected Matrix RuntimeValue, got {other:?}"),
    };
    let matrix = session.matrix_objects.resolve_owning(matrix_ref).expect("payload");
    assert_eq!(matrix.shape().rows, 2);
    assert_eq!(matrix.shape().cols, 2);
}

#[test]
fn compile_and_execute_elementwise_multiply_matrix_bindings_via_hadamard() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let b_term = session.builder().symbol("B", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let sb = match session.arena.get(b_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol B, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(1), Integer::from(2), Integer::from(3), Integer::from(4)]).expect("a"));
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(5), Integer::from(6), Integer::from(7), Integer::from(8)]).expect("b"));
    let product = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::ElementwiseMultiply),
        vec![a_term, b_term],
        Default::default(),
    );
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sb, matrix: b }),
            AthenaRequest::Term(product),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("ew mul");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: rows, .. }) if rows.len() == 2 => {
            let r0 = match session.arena.get(rows[0]) {
                Some(TermNode::Collection { elements: cells, .. }) => cells.clone(),
                other => panic!("row0: {other:?}"),
            };
            assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)));
            assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(12)));
        }
        other => panic!("expected Hadamard nested list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_elementwise_divide_matrix_bindings() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let b_term = session.builder().symbol("B", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let sb = match session.arena.get(b_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol B, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 2, vec![Integer::from(6), Integer::from(8)]).expect("a"));
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 2, vec![Integer::from(2), Integer::from(4)]).expect("b"));
    let quotient = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::ElementwiseDivide),
        vec![a_term, b_term],
        Default::default(),
    );
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sb, matrix: b }),
            AthenaRequest::Term(quotient),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("ew div");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Living 16: Dot projection flattens 1×n MatrixResult for elementwise divide.
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
        }
        other => panic!("expected flat quotient list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_elementwise_power_matrix_bindings() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let b_term = session.builder().symbol("B", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let sb = match session.arena.get(b_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol B, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 2, vec![Integer::from(2), Integer::from(3)]).expect("a"));
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 2, vec![Integer::from(3), Integer::from(2)]).expect("b"));
    let powered = session.builder().application(
        ApplicationHead::Semantic(SemanticOperator::ElementwisePower),
        vec![a_term, b_term],
        Default::default(),
    );
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sb, matrix: b }),
            AthenaRequest::Term(powered),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("ew pow");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(8)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(9)));
        }
        other => panic!("expected flat power list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_first_rest_flatten_matrix_bindings() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("a"));
    let first = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::First), vec![a_term], Default::default());

    let module = ExecutionCompiler::new()
        .compile(
            &mut session,
            &AthenaRequest::Control(ControlPlan::Sequence {
                steps: vec![
                    AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
                    AthenaRequest::Term(first),
                ],
            }),
        )
        .expect("first");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute first");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // First row of 2×2 projects as flat 1×2 Own surface.
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
        }
        other => panic!("expected First row list, got {other:?}"),
    }

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("a"));
    let rest = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Rest), vec![a_term], Default::default());
    let module = ExecutionCompiler::new()
        .compile(
            &mut session,
            &AthenaRequest::Control(ControlPlan::Sequence {
                steps: vec![
                    AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
                    AthenaRequest::Term(rest),
                ],
            }),
        )
        .expect("rest");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute rest");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Remaining single row is Own-surface 1×2 → flat list.
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4)));
        }
        other => panic!("expected Rest row list, got {other:?}"),
    }

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("a"));
    let flat = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Flatten), vec![a_term], Default::default());
    let module = ExecutionCompiler::new()
        .compile(
            &mut session,
            &AthenaRequest::Control(ControlPlan::Sequence {
                steps: vec![
                    AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
                    AthenaRequest::Term(flat),
                ],
            }),
        )
        .expect("flatten");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute flatten");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 4 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
            assert!(matches!(session.arena.get(cells[2]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
            assert!(matches!(session.arena.get(cells[3]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4)));
        }
        other => panic!("expected Flatten flat list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_most_reverse_matrix_bindings() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("a"));
    let most = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Most), vec![a_term], Default::default());
    let module = ExecutionCompiler::new()
        .compile(
            &mut session,
            &AthenaRequest::Control(ControlPlan::Sequence {
                steps: vec![
                    AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
                    AthenaRequest::Term(most),
                ],
            }),
        )
        .expect("most");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute most");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Most of 2×2 drops last row → 1×2 Own surface flat list.
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 2 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
        }
        other => panic!("expected Most row list, got {other:?}"),
    }

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 3, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
        ]).expect("a"));
    let rev = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Reverse), vec![a_term], Default::default());
    let module = ExecutionCompiler::new()
        .compile(
            &mut session,
            &AthenaRequest::Control(ControlPlan::Sequence {
                steps: vec![
                    AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
                    AthenaRequest::Term(rev),
                ],
            }),
        )
        .expect("reverse");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute reverse");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: cells, .. }) if cells.len() == 3 => {
            assert!(matches!(session.arena.get(cells[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
            assert!(matches!(session.arena.get(cells[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
            assert!(matches!(session.arena.get(cells[2]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
        }
        other => panic!("expected Reverse flat list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_det_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
        ]).expect("matrix"));
    let det = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Determinant), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(det),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("det matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(-2)));
}

#[test]
fn compile_and_execute_size_on_matrix_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 3, vec![
            Integer::from(1),
            Integer::from(2),
            Integer::from(3),
            Integer::from(4),
            Integer::from(5),
            Integer::from(6),
        ]).expect("matrix"));
    let size = session
        .builder()
        .application(ApplicationHead::Semantic(SemanticOperator::Size), vec![a_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix }),
            AthenaRequest::Term(size),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("size matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements, .. }) if elements.len() == 2 => {
            assert!(matches!(session.arena.get(elements[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
            assert!(matches!(session.arena.get(elements[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
        }
        other => panic!("expected Size dimensions list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_multiply_matrix_bindings_via_hadamard() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_ir::ApplicationHead;
    use athena_numeric::Integer;

    // Living 16: `Multiply` on two `RuntimeValue::Matrix` slots is Hadamard, not MatMul /
    // nested-List reverse recognition. MatMul requires an explicit Domain goal.
    let mut session = Session::new();
    let a_term = session.builder().symbol("A", Default::default());
    let b_term = session.builder().symbol("B", Default::default());
    let sa = match session.arena.get(a_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol A, got {other:?}"),
    };
    let sb = match session.arena.get(b_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol B, got {other:?}"),
    };
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(1), Integer::from(2), Integer::from(3), Integer::from(4)]).expect("a"));
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![Integer::from(5), Integer::from(6), Integer::from(7), Integer::from(8)]).expect("b"));
    let product = session.builder().application(ApplicationHead::Semantic(SemanticOperator::Multiply), vec![a_term, b_term], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a }),
            AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sb, matrix: b }),
            AthenaRequest::Term(product),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("mul matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    // Hadamard [[5,12],[21,32]]
    match session.arena.get(term) {
        Some(TermNode::Collection { elements: rows, .. }) if rows.len() == 2 => {
            let r0 = match session.arena.get(rows[0]) {
                Some(TermNode::Collection { elements: cells, .. }) => cells.clone(),
                other => panic!("row0: {other:?}"),
            };
            assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)));
            assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(12)));
        }
        other => panic!("expected Hadamard nested list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_define_deferred_evaluates_on_read() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(1, Default::default());
    let rhs = session.builder().application(plus, vec![a, b], Default::default());
    let sym_term = session.builder().symbol("a", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let request = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value: rhs,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::StoreResidualTerm,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define residual");
    ReferenceExecutor::new().execute(&mut session, &module).expect("define exec");
    assert!(session.defs.binding(symbol).is_none());
    assert_eq!(session.defs.residual_binding(symbol), Some(rhs));

    let read_module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(sym_term)).expect("read");
    let result_id = ReferenceExecutor::new().execute(&mut session, &read_module).expect("read exec");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected residual Plus[1,1] == 2, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_define_then_read_binding() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let sym_term = session.builder().symbol("y", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let value = session.builder().int(7, Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol,
        value,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let define_module = ExecutionCompiler::new().compile(&mut session, &define).expect("define");
    ReferenceExecutor::new().execute(&mut session, &define_module).expect("define exec");

    let read = AthenaRequest::Term(sym_term);
    let read_module = ExecutionCompiler::new().compile(&mut session, &read).expect("read");
    let result_id = ReferenceExecutor::new().execute(&mut session, &read_module).expect("read exec");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(value));
}

#[test]
fn compile_and_execute_sequence_define_read_clear() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let sym_term = session.builder().symbol("z", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol atom, got {other:?}"),
    };
    let value = session.builder().int(5, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::Define {
                symbol,
                value,
                kind: BindingKind::Session,
                evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
            }),
            AthenaRequest::Term(sym_term),
            AthenaRequest::Command(SessionCommand::ClearDefinition { symbol }),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("sequence");
    assert_eq!(module.regions[0].blocks.len(), 3);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    // 最后一步清除；结果为 Unit → Null 项。
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Null)) => {}
        other => panic!("expected Null after clear, got {other:?}"),
    }
    assert!(session.defs.binding(symbol).is_none());
}

#[test]
fn compile_and_execute_counted_loop_unroll() {
    let mut session = Session::new();
    let var = session.builder().symbol("i", Default::default());
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let iter = session.builder().list(vec![a, b, c], Default::default());
    let request = AthenaRequest::Control(ControlPlan::CountedLoop { variable: var, iterator: iter, body: Box::new(AthenaRequest::Term(var)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("counted");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(c));
    let symbol = match session.arena.get(var) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    assert_eq!(session.defs.binding(symbol), Some(c));
}

#[test]
fn compile_and_execute_iterate_collects_collection() {
    use athena_types::BindingEvaluationPolicy;

    let mut session = Session::new();
    let var = session.builder().symbol("i", Default::default());
    let a = session.builder().int(1, Default::default());
    let b = session.builder().int(2, Default::default());
    let c = session.builder().int(3, Default::default());
    let range = session.builder().list(vec![a, b, c], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Iterate {
        binder: var,
        range,
        body: Box::new(AthenaRequest::Term(var)),
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("iterate");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Collection { elements, .. }) => {
            assert_eq!(elements.len(), 3);
        }
        other => panic!("expected collection, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_index_scalar() {
    use athena_types::{IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let a = session.builder().int(10, Default::default());
    let b = session.builder().int(20, Default::default());
    let c = session.builder().int(30, Default::default());
    let list = session.builder().list(vec![a, b, c], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Index { target: list, axes: vec![IndexSpec::Scalar(IntegerIndex(2))] });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("index");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(20) => {}
        other => panic!("expected Index[..., 2] == 20, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_index_resolves_own_binding() {
    use athena_types::{BindingEvaluationPolicy, BindingKind, IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let ten = session.builder().int(10, Default::default());
    let twenty = session.builder().int(20, Default::default());
    let list = session.builder().list(vec![ten, twenty], Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol: match session.arena.get(a) {
            Some(TermNode::Atom(Atom::Symbol(s))) => *s,
            other => panic!("expected symbol A, got {other:?}"),
        },
        value: list,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let index = AthenaRequest::Control(ControlPlan::Index {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(2))],
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, index],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define+index");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(20) => {}
        other => panic!("expected Own A then Index[..., 2] == 20, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_index_free_symbol_becomes_apply() {
    use athena_types::{IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let speye = session.builder().symbol("speye", Default::default());
    let request = AthenaRequest::Control(ControlPlan::Index {
        target: speye,
        axes: vec![IndexSpec::Scalar(IntegerIndex(2))],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("index free");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(term) {
        Some(TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::ApplyHead),
            arguments,
        }) => {
            assert_eq!(arguments.len(), 2, "ApplyHead[speye, 2], got {arguments:?}");
            match session.arena.get(arguments[0]) {
                Some(TermNode::Atom(Atom::Symbol(_))) => {}
                other => panic!("expected speye symbol head, got {other:?}"),
            }
            match session.arena.get(arguments[1]) {
                Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
                other => panic!("expected arg 2, got {other:?}"),
            }
        }
        other => panic!("expected ApplyHead residual for free speye(2), got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_store_index_scalar() {
    use athena_types::{BindingEvaluationPolicy, BindingKind, IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let nine = session.builder().int(9, Default::default());
    let list = session.builder().list(vec![one, two, three], Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol: match session.arena.get(a) {
            Some(TermNode::Atom(Atom::Symbol(s))) => *s,
            other => panic!("expected symbol A, got {other:?}"),
        },
        value: list,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(2))],
        value: nine,
    });
    let read = AthenaRequest::Control(ControlPlan::Index {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(2))],
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, read],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("store index");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(9) => {}
        other => panic!("expected A(2) == 9 after StoreIndex, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_store_index_matrix_cell() {
    use athena_types::{BindingEvaluationPolicy, BindingKind, IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let m = session.builder().symbol("M", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let four = session.builder().int(4, Default::default());
    let nine = session.builder().int(9, Default::default());
    let r0 = session.builder().list(vec![one, two], Default::default());
    let r1 = session.builder().list(vec![three, four], Default::default());
    let matrix = session.builder().list(vec![r0, r1], Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol: match session.arena.get(m) {
            Some(TermNode::Atom(Atom::Symbol(s))) => *s,
            other => panic!("expected symbol M, got {other:?}"),
        },
        value: matrix,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: m,
        axes: vec![IndexSpec::Scalar(IntegerIndex(1)), IndexSpec::Scalar(IntegerIndex(2))],
        value: nine,
    });
    let read = AthenaRequest::Control(ControlPlan::Index {
        target: m,
        axes: vec![IndexSpec::Scalar(IntegerIndex(1)), IndexSpec::Scalar(IntegerIndex(2))],
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, read],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("store matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(9) => {}
        other => panic!("expected M(1,2) == 9 after StoreIndex, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_store_index_grow_flat() {
    use athena_types::{BindingEvaluationPolicy, BindingKind, IndexSpec, IntegerIndex, IntegerOffset};

    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let five = session.builder().int(5, Default::default());
    let list = session.builder().list(vec![one, two, three], Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol: match session.arena.get(a) {
            Some(TermNode::Atom(Atom::Symbol(s))) => *s,
            other => panic!("expected symbol A, got {other:?}"),
        },
        value: list,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    // A(end+1)=5 → [1, 2, 3, 5]
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: a,
        axes: vec![IndexSpec::EndRelative(IntegerOffset(1))],
        value: five,
    });
    let read = AthenaRequest::Term(a);
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, read],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("store grow");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements, .. }) => {
            assert_eq!(elements.len(), 4);
            let vals: Vec<_> = elements
                .iter()
                .map(|e| match session.arena.get(*e) {
                    Some(TermNode::Atom(Atom::Number(n))) => n.as_exact_integer().expect("int"),
                    other => panic!("expected int, got {other:?}"),
                })
                .collect();
            assert_eq!(vals, vec![1, 2, 3, 5]);
        }
        other => panic!("expected grown list, got {other:?}"),
    }

    // A(5)=9 on [1,2,3] pads with 0 → [1,2,3,0,9]
    let mut session = Session::new();
    let a = session.builder().symbol("A", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let nine = session.builder().int(9, Default::default());
    let list = session.builder().list(vec![one, two, three], Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol: match session.arena.get(a) {
            Some(TermNode::Atom(Atom::Symbol(s))) => *s,
            other => panic!("expected symbol A, got {other:?}"),
        },
        value: list,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: a,
        axes: vec![IndexSpec::Scalar(IntegerIndex(5))],
        value: nine,
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, AthenaRequest::Term(a)],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("store pad");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements, .. }) => {
            let vals: Vec<_> = elements
                .iter()
                .map(|e| match session.arena.get(*e) {
                    Some(TermNode::Atom(Atom::Number(n))) => n.as_exact_integer().expect("int"),
                    other => panic!("expected int, got {other:?}"),
                })
                .collect();
            assert_eq!(vals, vec![1, 2, 3, 0, 9]);
        }
        other => panic!("expected padded list, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_store_index_grow_matrix() {
    use athena_types::{BindingEvaluationPolicy, BindingKind, IndexSpec, IntegerIndex};

    let mut session = Session::new();
    let m = session.builder().symbol("M", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let four = session.builder().int(4, Default::default());
    let nine = session.builder().int(9, Default::default());
    let r0 = session.builder().list(vec![one, two], Default::default());
    let r1 = session.builder().list(vec![three, four], Default::default());
    let matrix = session.builder().list(vec![r0, r1], Default::default());
    let define = AthenaRequest::Command(SessionCommand::Define {
        symbol: match session.arena.get(m) {
            Some(TermNode::Atom(Atom::Symbol(s))) => *s,
            other => panic!("expected symbol M, got {other:?}"),
        },
        value: matrix,
        kind: BindingKind::Session,
        evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
    });
    // M(3,3)=9 on 2×2 → 3×3 with zero pad
    let store = AthenaRequest::Control(ControlPlan::StoreIndex {
        target: m,
        axes: vec![IndexSpec::Scalar(IntegerIndex(3)), IndexSpec::Scalar(IntegerIndex(3))],
        value: nine,
    });
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![define, store, AthenaRequest::Term(m)],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("grow matrix");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: rows, .. }) => {
            assert_eq!(rows.len(), 3);
            let mut grid = Vec::new();
            for row in rows.clone() {
                match session.arena.get(row) {
                    Some(TermNode::Collection { elements: cols, .. }) => {
                        assert_eq!(cols.len(), 3);
                        for c in cols {
                            match session.arena.get(*c) {
                                Some(TermNode::Atom(Atom::Number(n))) => {
                                    grid.push(n.as_exact_integer().expect("int"));
                                }
                                other => panic!("expected int cell, got {other:?}"),
                            }
                        }
                    }
                    other => panic!("expected row collection, got {other:?}"),
                }
            }
            assert_eq!(grid, vec![1, 2, 0, 3, 4, 0, 0, 0, 9]);
        }
        other => panic!("expected grown matrix, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_index_column_major_flatten() {
    use athena_types::IndexSpec;

    let mut session = Session::new();
    // [1, 2; 3, 4] → column-major flatten [1; 3; 2; 4]
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let four = session.builder().int(4, Default::default());
    let r0 = session.builder().list(vec![one, two], Default::default());
    let r1 = session.builder().list(vec![three, four], Default::default());
    let matrix = session.builder().list(vec![r0, r1], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Index {
        target: matrix,
        axes: vec![IndexSpec::ColumnMajorFlatten],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("flatten");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Collection { elements: rows, .. }) => {
            assert_eq!(rows.len(), 4, "expected 4×1 column vector");
            let rows = rows.clone();
            let mut vals = Vec::with_capacity(4);
            for row in rows {
                match session.arena.get(row) {
                    Some(TermNode::Collection { elements: cols, .. }) => {
                        assert_eq!(cols.len(), 1);
                        match session.arena.get(cols[0]) {
                            Some(TermNode::Atom(Atom::Number(n))) => vals.push(n.as_exact_integer().expect("int")),
                            other => panic!("expected number cell, got {other:?}"),
                        }
                    }
                    other => panic!("expected row collection, got {other:?}"),
                }
            }
            assert_eq!(vals, vec![1, 3, 2, 4]);
        }
        other => panic!("expected flattened collection, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_term_counted_loop_range() {
    let mut session = Session::new();
    let var = session.builder().symbol("i", Default::default());
    let one = session.builder().int(1, Default::default());
    let three = session.builder().int(3, Default::default());
    let range_op = ApplicationHead::Semantic(SemanticOperator::Range);
    let iter = session.builder().application(range_op, vec![one, three], Default::default());
    let request = AthenaRequest::Control(ControlPlan::CountedLoop { variable: var, iterator: iter, body: Box::new(AthenaRequest::Term(var)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("counted range");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3) => {}
        other => panic!("expected CountedLoop range last value == 3, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_loop_while_false() {
    let mut session = Session::new();
    let cond = session.builder().boolean(false, Default::default());
    let body = session.builder().int(1, Default::default());
    let request = AthenaRequest::Control(ControlPlan::LoopWhile { condition: cond, body: Box::new(AthenaRequest::Term(body)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("loop");
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::BudgetCheck)));
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Null)) => {}
        other => panic!("expected Unit/Null after zero-trip loop, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_term_loop_while_zero() {
    let mut session = Session::new();
    let zero = session.builder().int(0, Default::default());
    let body = session.builder().int(1, Default::default());
    let request = AthenaRequest::Control(ControlPlan::LoopWhile { condition: zero, body: Box::new(AthenaRequest::Term(body)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("loop control");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Null)) => {}
        other => panic!("expected Null after LoopWhile[0,1], got {other:?}"),
    }
}

#[test]
fn compile_and_execute_goal_call_provider_dispatches_domain() {
    use athena_engine::{
        api::request::DomainGoal,
        domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
        execution::execute_ir_request,
        runtime::values::RuntimeValue,
    };
    use athena_numeric::Integer;

    let mut session = Session::new();
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
        a: Integer::from_i64(12),
        b: Integer::from_i64(8),
    })));
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("goal");
    assert_eq!(module.provider_calls.len(), 1);
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::CallProvider)));
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::PublishResult)));
    let result_id = execute_ir_request(&mut session, request).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    let value_id = loaded.value.expect("value");
    match session.values.get(value_id).expect("runtime") {
        RuntimeValue::Domain(athena_engine::domains::dispatch::DomainResult::NumberTheory(
            athena_engine::domains::number_theory::NumberTheoryResult::Exact {
                value: athena_engine::domains::number_theory::NumberTheoryValue::Integer(n),
            },
        )) => assert_eq!(n, &Integer::from_i64(4)),
        other => panic!("expected NumberTheory Exact Integer gcd, got {other:?}"),
    }
}

#[test]
fn call_provider_without_payload_hard_fails() {
    use athena_engine::{
        api::request::DomainGoal,
        domains::{dispatch::DomainRequest, number_theory::NumberTheoryRequest},
    };
    use athena_numeric::Integer;

    let mut session = Session::new();
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::NumberTheory(NumberTheoryRequest::Gcd {
        a: Integer::from_i64(12),
        b: Integer::from_i64(8),
    })));
    let mut module = ExecutionCompiler::new().compile(&mut session, &request).expect("goal");
    assert_eq!(module.provider_calls.len(), 1);
    // 模拟未绑定 payload 的 CallProvider：不得靠 host 侧通道补救。
    module.provider_calls[0].payload = None;
    module.fingerprint = athena_engine::execution::ir::ModuleFingerprint::of_module(&module);
    let err = ReferenceExecutor::new().execute(&mut session, &module).expect_err("unbound payload must hard-fail");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("provider_payload_unbound"));
}

#[test]
fn compile_and_execute_recover_success_body() {
    let mut session = Session::new();
    let body = session.builder().int(8, Default::default());
    let handler = session.builder().int(9, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Recover {
        body: Box::new(AthenaRequest::Term(body)),
        handler: Box::new(AthenaRequest::Term(handler)),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("recover");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(body));
}

#[test]
fn compile_and_execute_recover_reject_and_success() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let err_req = AthenaRequest::Control(ControlPlan::Recover {
        body: Box::new(AthenaRequest::Control(ControlPlan::Reject)),
        handler: Box::new(AthenaRequest::Term(one)),
    });
    let err_mod = ExecutionCompiler::new().compile(&mut session, &err_req).expect("recover reject");
    let err_id = ReferenceExecutor::new().execute(&mut session, &err_mod).expect("err exec");
    let err_out = session.results.get(err_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(err_out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1) => {}
        other => panic!("expected Recover[Reject,1] == 1, got {other:?}"),
    }

    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let ok_req = AthenaRequest::Control(ControlPlan::Recover {
        body: Box::new(AthenaRequest::Term(two)),
        handler: Box::new(AthenaRequest::Term(three)),
    });
    let ok_mod = ExecutionCompiler::new().compile(&mut session, &ok_req).expect("recover ok");
    let ok_id = ReferenceExecutor::new().execute(&mut session, &ok_mod).expect("ok exec");
    let ok_out = session.results.get(ok_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(ok_out) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2) => {}
        other => panic!("expected Recover[2,3] == 2, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_cond_second_arm() {
    let mut session = Session::new();
    let c0 = session.builder().boolean(false, Default::default());
    let c1 = session.builder().boolean(true, Default::default());
    let a0 = session.builder().int(10, Default::default());
    let a1 = session.builder().int(20, Default::default());
    let otherwise = session.builder().int(30, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Cond {
        arms: vec![(c0, Box::new(AthenaRequest::Term(a0))), (c1, Box::new(AthenaRequest::Term(a1)))],
        otherwise: Some(Box::new(AthenaRequest::Term(otherwise))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("cond");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(a1));
}

#[test]
fn compile_and_execute_local_scope_body() {
    let mut session = Session::new();
    let term = session.builder().int(11, Default::default());
    let request = AthenaRequest::Control(ControlPlan::LocalScope { body: Box::new(AthenaRequest::Term(term)) });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope");
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::EnterScope)));
    assert!(module.effect_edges.iter().any(|e| matches!(e.kind, EffectKind::ExitScope)));
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(term));
}

#[test]
fn compile_and_execute_local_scope_shadows_session() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let sym_term = session.builder().symbol("s", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let global = session.builder().int(1, Default::default());
    let local = session.builder().int(2, Default::default());
    session.defs.write_binding(symbol, global);

    let request = AthenaRequest::Control(ControlPlan::LocalScope {
        body: Box::new(AthenaRequest::Control(ControlPlan::Sequence {
            steps: vec![
                AthenaRequest::Command(SessionCommand::Define {
                    symbol,
                    value: local,
                    kind: BindingKind::Session,
                    evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                }),
                AthenaRequest::Term(sym_term),
            ],
        })),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    assert_eq!(loaded.symbolic_term, Some(local));
    // 局部作用域退出后 Session Own 不变。
    assert_eq!(session.defs.binding(symbol), Some(global));
}

#[test]
fn compile_and_execute_local_scope_clear_hides_session_own() {
    use athena_engine::api::request::SessionCommand;

    let mut session = Session::new();
    let sym_term = session.builder().symbol("b", Default::default());
    let symbol = match session.arena.get(sym_term) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let global = session.builder().int(5, Default::default());
    session.defs.write_binding(symbol, global);

    let request = AthenaRequest::Control(ControlPlan::DynamicScope {
        body: Box::new(AthenaRequest::Control(ControlPlan::Sequence {
            steps: vec![
                AthenaRequest::Command(SessionCommand::ClearDefinition { symbol }),
                AthenaRequest::Term(sym_term),
            ],
        })),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("scope clear");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Symbol(id))) if *id == symbol => {}
        other => panic!("expected cleared local to yield symbol b, got {other:?}"),
    }
    assert_eq!(session.defs.binding(symbol), Some(global));
}

#[test]
fn compile_and_execute_boolean_not_and() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let f = session.builder().boolean(false, Default::default());
    let and = ApplicationHead::Semantic(SemanticOperator::And);
    let not = ApplicationHead::Semantic(SemanticOperator::Not);
    let and_term = session.builder().application(and, vec![t, f], Default::default());
    let term = session.builder().application(not, vec![and_term], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("bool ops");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    let out = loaded.symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected Not[And[True,False]] == True, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_control_branch_boolean() {
    let mut session = Session::new();
    let cond = session.builder().boolean(true, Default::default());
    let then_term = session.builder().int(11, Default::default());
    let else_term = session.builder().int(22, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Branch {
        condition: cond,
        then_branch: Box::new(AthenaRequest::Term(then_term)),
        else_branch: Some(Box::new(AthenaRequest::Term(else_term))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("branch");
    assert_eq!(module.regions[0].blocks.len(), 3);
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(then_term));
}

#[test]
fn compile_and_execute_sequence_and_hold() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![AthenaRequest::Term(one), AthenaRequest::Term(two), AthenaRequest::Term(three)],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("sequence");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(three));

    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let hold = ApplicationHead::Semantic(SemanticOperator::Hold);
    let inner = session.builder().application(plus, vec![one, one], Default::default());
    let held = session.builder().application(hold, vec![inner], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(held)).expect("hold");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Application { head, arguments })
            if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Hold))
                && arguments.len() == 1
                && session.arena.structural_eq(arguments[0], inner) => {}
        other => panic!("expected Hold[Add[1,1]] unevaluated, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_cond_picks_true_arm() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let three = session.builder().int(3, Default::default());
    let fals = session.builder().boolean(false, Default::default());
    let tru = session.builder().boolean(true, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Cond {
        arms: vec![
            (fals, Box::new(AthenaRequest::Term(one))),
            (tru, Box::new(AthenaRequest::Term(two))),
            (tru, Box::new(AthenaRequest::Term(three))),
        ],
        otherwise: None,
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("cond");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(two));
}

#[test]
fn compile_and_execute_define_in_sequence() {
    use athena_engine::api::request::SessionCommand;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let symbol = match session.arena.get(x) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let five = session.builder().int(5, Default::default());
    let one = session.builder().int(1, Default::default());
    let plus = ApplicationHead::Semantic(SemanticOperator::Add);
    let use_x = session.builder().application(plus, vec![x, one], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Sequence {
        steps: vec![
            AthenaRequest::Command(SessionCommand::Define {
                symbol,
                value: five,
                kind: BindingKind::Session,
                evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
            }),
            AthenaRequest::Term(use_x),
        ],
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("define seq");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(6) => {}
        other => panic!("expected Define then Add == 6, got {other:?}"),
    }
}

#[test]
fn compile_and_execute_branch_then_sequence_with_define() {
    use athena_engine::api::request::SessionCommand;
    use athena_engine::execution::execute_ir_request;
    use athena_types::{BindingEvaluationPolicy, BindingKind};

    let mut session = Session::new();
    let z = session.builder().symbol("z", Default::default());
    let symbol = match session.arena.get(z) {
        Some(TermNode::Atom(Atom::Symbol(id))) => *id,
        other => panic!("expected symbol, got {other:?}"),
    };
    let seven = session.builder().int(7, Default::default());
    let tru = session.builder().boolean(true, Default::default());
    let fals = session.builder().boolean(false, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Branch {
        condition: tru,
        then_branch: Box::new(AthenaRequest::Control(ControlPlan::Sequence {
            steps: vec![
                AthenaRequest::Command(SessionCommand::Define {
                    symbol,
                    value: seven,
                    kind: BindingKind::Session,
                    evaluation: BindingEvaluationPolicy::EvaluateBeforeStore,
                }),
                AthenaRequest::Term(tru),
            ],
        })),
        else_branch: Some(Box::new(AthenaRequest::Term(fals))),
    });
    let result_id = execute_ir_request(&mut session, request).expect("branch+sequence");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected True from then Sequence, got {other:?}"),
    }
    assert_eq!(session.defs.binding(symbol), Some(seven));
}

#[test]
fn compile_and_execute_runtime_branch() {
    let mut session = Session::new();
    let one = session.builder().int(1, Default::default());
    let seven = session.builder().int(7, Default::default());
    let eight = session.builder().int(8, Default::default());
    let equal = ApplicationHead::Semantic(SemanticOperator::Equal);
    let cond = session.builder().application(equal, vec![one, one], Default::default());
    let request = AthenaRequest::Control(ControlPlan::Branch {
        condition: cond,
        then_branch: Box::new(AthenaRequest::Term(seven)),
        else_branch: Some(Box::new(AthenaRequest::Term(eight))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("branch");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(seven));

    let fals = session.builder().boolean(false, Default::default());
    let request = AthenaRequest::Control(ControlPlan::Branch {
        condition: fals,
        then_branch: Box::new(AthenaRequest::Term(seven)),
        else_branch: Some(Box::new(AthenaRequest::Term(eight))),
    });
    let module = ExecutionCompiler::new().compile(&mut session, &request).expect("branch false");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    assert_eq!(session.results.get(result_id).expect("result").symbolic_term, Some(eight));
}

#[test]
fn compile_and_execute_sameq_and_trueq() {
    let mut session = Session::new();
    let t = session.builder().boolean(true, Default::default());
    let f = session.builder().boolean(false, Default::default());
    let same = ApplicationHead::Semantic(SemanticOperator::Identical);
    let true_q = ApplicationHead::Semantic(SemanticOperator::TrueQ);
    let same_term = session.builder().application(same, vec![t, f], Default::default());
    let term = session.builder().application(true_q, vec![same_term], Default::default());
    // `TrueQ[SameQ[True,False]]` == `TrueQ[False]` == `False`
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(term)).expect("sameq");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Boolean(false))) => {}
        other => panic!("expected False, got {other:?}"),
    }

    let a = session.builder().int(3, Default::default());
    let b = session.builder().int(3, Default::default());
    let eq = ApplicationHead::Semantic(SemanticOperator::Equal);
    let eq_term = session.builder().application(eq, vec![a, b], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(eq_term)).expect("equal");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let loaded = session.results.get(result_id).expect("result");
    match session.arena.get(loaded.symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Boolean(true))) => {}
        other => panic!("expected Equal[3,3] == True, got {other:?}"),
    }
}

#[test]
fn equal_symbolic_stays_residual_identical_is_structural() {
    use athena_ir::{ApplicationHead, SemanticOperator, TermNode};
    let mut session = Session::new();
    let x = session.builder().symbol("x", Default::default());
    let one = session.builder().int(1, Default::default());
    let two = session.builder().int(2, Default::default());
    let pow = session.builder().application(ApplicationHead::Semantic(SemanticOperator::Power), vec![x, two], Default::default());
    let eq = session.builder().application(ApplicationHead::Semantic(SemanticOperator::Equal), vec![pow, one], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(eq)).expect("eq");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    let out = session.results.get(result_id).expect("result").symbolic_term.expect("term");
    match session.arena.get(out) {
        Some(TermNode::Application { head, .. }) if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Equal)) => {}
        other => panic!("expected residual Equal, got {other:?}"),
    }
    let same = session.builder().application(ApplicationHead::Semantic(SemanticOperator::Identical), vec![one, two], Default::default());
    let module = ExecutionCompiler::new().compile(&mut session, &AthenaRequest::Term(same)).expect("same");
    let result_id = ReferenceExecutor::new().execute(&mut session, &module).expect("execute");
    match session.arena.get(session.results.get(result_id).expect("result").symbolic_term.expect("term")) {
        Some(TermNode::Atom(Atom::Boolean(false))) => {}
        other => panic!("expected Identical false, got {other:?}"),
    }
}
