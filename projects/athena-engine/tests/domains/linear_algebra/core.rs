//! 矩阵不变量、精确/机器双路径与中性请求合同。

use athena_engine::{
    domains::{
        DomainRequest, DomainResult, execute_domain,
        linear_algebra::{
            AlgorithmGuarantee, IndexSpec, LinearAlgebraRequest, LinearAlgebraResult, LinearAlgebraValue, MatrixEntry, MatrixEqualityKind,
            MatrixParent, MatrixShape, MatrixValue, SolveDisposition, StorageOrder, det_bareiss, execute_linear_algebra, hadamard, matmul,
            matrices_equal, rank_exact, scalar_index_from_one_based, solve_exact, solve_machine, transpose,
        },
    },
    runtime::Session,
};
use athena_numeric::{Integer, Rational};

fn i(n: i64) -> Integer {
    Integer::from_i64(n)
}

fn q(n: i64, d: i64) -> Rational {
    Rational::new(Integer::from_i64(n), Integer::from_i64(d))
}

#[test]
fn l0_empty_and_zero_dim_shapes() {
    let z = MatrixValue::zeros(MatrixParent::integers(), MatrixShape::new(0, 3), StorageOrder::RowMajor).unwrap();
    assert!(z.shape().is_empty());
    assert_eq!(z.shape().element_count().unwrap(), 0);
    let sq = MatrixValue::zeros(MatrixParent::rationals(), MatrixShape::new(0, 0), StorageOrder::RowMajor).unwrap();
    assert!(sq.shape().is_square());
}

#[test]
fn l0_transpose_view_shares_buffer_and_cow_on_write() {
    let m = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let t = transpose(&m);
    assert_eq!(t.shape(), MatrixShape::new(2, 2).transpose());
    assert!(m.buffer_strong_count() >= 2);
    assert_eq!(t.get(0, 1).unwrap(), MatrixEntry::Integer(i(3)));
    let mut owned = m.owning_copy();
    owned.set_owned(0, 0, MatrixEntry::Integer(i(9))).unwrap();
    assert_eq!(m.get(0, 0).unwrap(), MatrixEntry::Integer(i(1)));
    assert_eq!(owned.get(0, 0).unwrap(), MatrixEntry::Integer(i(9)));
}

#[test]
fn l0_matmul_and_hadamard_shape_checks() {
    let a = MatrixValue::from_f64_row_major(2, 3, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(3, 2, vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0]).unwrap();
    let c = matmul(&a, &b).unwrap();
    assert_eq!(c.shape(), MatrixShape::new(2, 2));
    assert!(hadamard(&a, &b).is_err());
    let h = hadamard(&a, &a).unwrap();
    assert_eq!(h.get(0, 0).unwrap(), MatrixEntry::MachineF64(1.0));
}

#[test]
fn exact_and_machine_buffers_are_incompatible() {
    let exact = MatrixParent::rationals();
    let machine = MatrixParent::machine_real();
    assert!(!exact.buffer_compatible_with(machine));
    let a = MatrixValue::from_rationals_row_major(1, 1, vec![q(1, 1)]).unwrap();
    let b = MatrixValue::from_f64_row_major(1, 1, vec![1.0]).unwrap();
    assert!(matmul(&a, &b).is_err());
}

#[test]
fn neutral_matmul_and_hadamard_requests_are_distinct() {
    let mut store = athena_engine::domains::linear_algebra::MatrixObjectStore::new();
    let a = store.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let b = store.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(5), i(6), i(7), i(8)]).unwrap());

    let mm = LinearAlgebraRequest::MatMul { lhs: a.into(), rhs: b.into() };
    let had = LinearAlgebraRequest::Hadamard { lhs: a.into(), rhs: b.into() };
    assert_ne!(mm, had);

    let r1 = execute_linear_algebra(mm.owning_copy(), &store);
    let r2 = execute_linear_algebra(mm, &store);
    assert_eq!(r1, r2);
    let _ = execute_linear_algebra(had, &store);
}

#[test]
fn one_based_index_helper_builds_neutral_request() {
    let mut store = athena_engine::domains::linear_algebra::MatrixObjectStore::new();
    let m = store.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(10), i(20), i(30), i(40)]).unwrap());
    let spec = scalar_index_from_one_based(2, 1).unwrap();
    assert_eq!(spec, IndexSpec::Scalar { row: 1, col: 0 });

    let IndexSpec::Scalar { row, col } = spec
    else {
        panic!("scalar");
    };
    let req = LinearAlgebraRequest::Index { matrix: m, row, col };
    let LinearAlgebraResult::Ok { value: LinearAlgebraValue::Matrix(v) } = execute_linear_algebra(req, &store)
    else {
        panic!("index");
    };
    assert_eq!(v.get(0, 0).unwrap(), MatrixEntry::Integer(i(30)));
}

#[test]
fn l1_exact_rank_det_solve_unique() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let rank = rank_exact(&a).unwrap();
    assert_eq!(rank.rank, 2);
    assert_eq!(rank.guarantee, AlgorithmGuarantee::Exact);

    let det = det_bareiss(&a).unwrap();
    assert_eq!(det.det, q(-2, 1));

    let b = MatrixValue::from_integers_row_major(2, 1, vec![i(5), i(11)]).unwrap();
    let sol = solve_exact(&a, &b).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Unique);
    let x = sol.particular.unwrap();
    // [1,2;3,4][1;2]=[5;11]
    assert!(
        matrices_equal(
            &x,
            &MatrixValue::from_rationals_row_major(2, 1, vec![q(1, 1), q(2, 1)]).unwrap(),
            MatrixEqualityKind::ExactMathematical
        )
        .unwrap()
    );
}

#[test]
fn l1_exact_solve_inconsistent_and_infinite() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap();
    let b_bad = MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(0)]).unwrap();
    let bad = solve_exact(&a, &b_bad).unwrap();
    assert_eq!(bad.disposition, SolveDisposition::Inconsistent);

    let b_ok = MatrixValue::from_integers_row_major(2, 1, vec![i(2), i(4)]).unwrap();
    let inf = solve_exact(&a, &b_ok).unwrap();
    assert!(matches!(inf.disposition, SolveDisposition::Infinite { .. }));
}

#[test]
fn l1_machine_solve_with_residual() {
    let a = MatrixValue::from_f64_row_major(2, 2, vec![3.0, 1.0, 1.0, 2.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(2, 1, vec![9.0, 8.0]).unwrap();
    let sol = solve_machine(&a, &b, 1e-12).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Unique);
    let w = sol.witness.unwrap();
    assert!(w.residual_inf < 1e-9);
    assert_eq!(w.numerical_rank, 2);
    assert_eq!(sol.guarantee, AlgorithmGuarantee::Approximate);
}

#[test]
fn domain_request_dispatches_linear_algebra() {
    let mut session = Session::new();
    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 1, vec![i(7)]).unwrap());
    let req = DomainRequest::LinearAlgebra(LinearAlgebraRequest::Det { matrix: a.into() });
    let DomainResult::LinearAlgebra(LinearAlgebraResult::Ok { value: LinearAlgebraValue::ExactDet(d) }) =
        execute_domain(&mut session, req).unwrap()
    else {
        panic!("expected exact det");
    };
    assert_eq!(d.det, q(7, 1));
}

#[test]
fn goal_transpose_projects_nested_list_via_execution() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Transpose {
        matrix: matrix.into(),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("transpose goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    // {{1, 3}, {2, 4}}
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    assert_eq!(rows.len(), 2);
    let TermNode::Collection { elements: r0, .. } = session.arena.get(rows[0]).expect("r0")
    else {
        panic!("row0");
    };
    let TermNode::Collection { elements: r1, .. } = session.arena.get(rows[1]).expect("r1")
    else {
        panic!("row1");
    };
    assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
    assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
    assert!(matches!(session.arena.get(r1[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(2)));
    assert!(matches!(session.arena.get(r1[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(4)));
}

#[test]
fn goal_transpose_resolves_matrix_binding_at_execute_time() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal, SessionCommand},
        domains::linear_algebra::MatrixOperand,
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::SymbolId;

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let symbol = SymbolId(9);
    let define = AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix });
    execute_ir_request(&mut session, define).expect("define matrix");
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Transpose {
        matrix: MatrixOperand::binding(symbol),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("transpose binding");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    assert_eq!(rows.len(), 2);
    let TermNode::Collection { elements: r0, .. } = session.arena.get(rows[0]).expect("r0")
    else {
        panic!("row0");
    };
    assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
    assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
}

#[test]
fn goal_det_resolves_matrix_binding_at_execute_time() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal, SessionCommand},
        domains::linear_algebra::MatrixOperand,
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::SymbolId;

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let symbol = SymbolId(11);
    execute_ir_request(&mut session, AthenaRequest::Command(SessionCommand::DefineMatrix { symbol, matrix })).expect("define");
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Det {
        matrix: MatrixOperand::binding(symbol),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("det binding");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(-2)));
}

#[test]
fn goal_dot_resolves_matrix_bindings_at_execute_time() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal, SessionCommand},
        domains::linear_algebra::MatrixOperand,
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::SymbolId;

    let mut session = Session::new();
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(1)]).unwrap());
    let sa = SymbolId(21);
    let sb = SymbolId(22);
    execute_ir_request(&mut session, AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sa, matrix: a })).expect("define a");
    execute_ir_request(&mut session, AthenaRequest::Command(SessionCommand::DefineMatrix { symbol: sb, matrix: b })).expect("define b");
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Dot {
        lhs: MatrixOperand::binding(sa),
        rhs: MatrixOperand::binding(sb),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("dot binding");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: items, .. } = session.arena.get(term).expect("list")
    else {
        panic!("expected flat list");
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(session.arena.get(items[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
    assert!(matches!(session.arena.get(items[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(7)));
}

#[test]
fn goal_rank_projects_integer_via_execution() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Rank { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("rank goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
}

#[test]
fn goal_rref_projects_nested_list_via_execution() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Rref { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("rref goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    // Identity for invertible 2x2
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    assert_eq!(rows.len(), 2);
    let TermNode::Collection { elements: r0, .. } = session.arena.get(rows[0]).expect("r0")
    else {
        panic!("row0");
    };
    let TermNode::Collection { elements: r1, .. } = session.arena.get(rows[1]).expect("r1")
    else {
        panic!("row1");
    };
    assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
    assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    assert!(matches!(session.arena.get(r1[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    assert!(matches!(session.arena.get(r1[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
}

#[test]
fn goal_inverse_projects_nested_list_via_execution() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(0), i(1)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Inverse { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("inverse goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    assert_eq!(rows.len(), 2);
    let TermNode::Collection { elements: r0, .. } = session.arena.get(rows[0]).expect("r0")
    else {
        panic!("row0");
    };
    let TermNode::Collection { elements: r1, .. } = session.arena.get(rows[1]).expect("r1")
    else {
        panic!("row1");
    };
    assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
    assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    assert!(matches!(session.arena.get(r1[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    assert!(matches!(session.arena.get(r1[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
}

#[test]
fn goal_trace_projects_integer_via_execution() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Trace { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("trace goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)));
}

#[test]
fn goal_dot_matrix_vector_projects_flat_list() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    // Explicit column vector — dialects must choose orientation, kernel does not guess from `1×n`.
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(1)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Dot { lhs: a.into(), rhs: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("dot goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: items, .. } = session.arena.get(term).expect("list")
    else {
        panic!("expected flat list");
    };
    assert_eq!(items.len(), 2);
    assert!(matches!(session.arena.get(items[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
    assert!(matches!(session.arena.get(items[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(7)));
}

#[test]
fn goal_dot_rejects_row_vector_without_explicit_orientation() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 2, vec![i(1), i(1)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Dot { lhs: a.into(), rhs: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("shape errors become results");
    let result = session.results.get(result_id).expect("result");
    assert_ne!(result.status, ComputationStatus::Exact, "incompatible 2x2·1x2 must not succeed as Exact");
    assert!(
        result.diagnostics.iter().any(|d| d.code == athena_types::DiagnosticCode::ShapeMismatch)
            || result.symbolic_term.is_none(),
        "expected ShapeMismatch diagnostic or no successful projection, got status={:?} diags={:?}",
        result.status,
        result.diagnostics
    );
}

#[test]
fn goal_cross_projects_flat_list() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 3, vec![i(1), i(0), i(0)]).unwrap());
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 3, vec![i(0), i(1), i(0)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Cross { lhs: a.into(), rhs: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("cross goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: items, .. } = session.arena.get(term).expect("list")
    else {
        panic!("expected flat list");
    };
    assert_eq!(items.len(), 3);
    assert!(matches!(session.arena.get(items[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    assert!(matches!(session.arena.get(items[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    assert!(matches!(session.arena.get(items[2]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
}

#[test]
fn goal_nullspace_rank1_projects_row_basis() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::NullSpace { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("nullspace goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    assert_eq!(rows.len(), 1);
    let TermNode::Collection { elements: r0, .. } = session.arena.get(rows[0]).expect("r0")
    else {
        panic!("row0");
    };
    assert_eq!(r0.len(), 2);
    assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(-2)));
    assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
}

#[test]
fn goal_norm_projects_integer() {
    use athena_engine::{api::{AthenaRequest, DomainGoal}, execution::execute_ir_request};
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(1, 2, vec![i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Norm { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("norm goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)));
}
