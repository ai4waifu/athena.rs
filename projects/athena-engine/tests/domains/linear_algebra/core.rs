//! 矩阵不变量、精确/机器双路径与中性请求合同。

use athena_engine::{
    domains::{
        DomainRequest, DomainResult, execute_domain,
        linear_algebra::{
            AlgorithmGuarantee, IndexSpec, LinearAlgebraRequest, LinearAlgebraResult, LinearAlgebraValue, MatrixEntry, MatrixEqualityKind,
            MatrixParent, MatrixShape, MatrixValue, SolveDisposition, StorageOrder, conjugate_transpose, det_bareiss, elementwise_divide, execute_linear_algebra, flatten_row_major, reverse_matrix, join_matrices, slice_matrix,
            hadamard, is_diagonal, is_lower_triangular, is_symmetric, kronecker, matmul, matrices_equal, rank_exact, right_solve_exact,
            scalar_index_from_one_based, solve_exact, solve_machine, transpose, tril, triu,
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
fn l0_conjugate_transpose_matches_transpose_on_reals() {
    let m = MatrixValue::from_integers_row_major(2, 3, vec![i(1), i(2), i(3), i(4), i(5), i(6)]).expect("m");
    let t = transpose(&m);
    let ct = conjugate_transpose(&m);
    assert_eq!(t.shape(), ct.shape());
    assert!(matrices_equal(&t, &ct, MatrixEqualityKind::Structural).unwrap());
}

#[test]
fn l0_conjugate_transpose_negates_imag_on_complex_exact() {
    let m = MatrixValue::from_complex_exact_row_major(
        2,
        2,
        vec![(q(1, 1), q(2, 1)), (q(3, 1), q(4, 1)), (q(5, 1), q(6, 1)), (q(7, 1), q(8, 1))],
    )
    .expect("m");
    let ct = conjugate_transpose(&m);
    assert_eq!(ct.shape(), MatrixShape::new(2, 2));
    assert_eq!(
        ct.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(1, 1),
            im: q(-2, 1),
        }
    );
    assert_eq!(
        ct.get(0, 1).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(5, 1),
            im: q(-6, 1),
        }
    );
    assert_eq!(
        ct.get(1, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(3, 1),
            im: q(-4, 1),
        }
    );
    assert_eq!(
        ct.get(1, 1).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(7, 1),
            im: q(-8, 1),
        }
    );
}

#[test]
fn l0_complex_exact_matmul_and_hadamard() {
    let a = MatrixValue::from_complex_exact_row_major(
        2,
        2,
        vec![(q(1, 1), q(1, 1)), (q(0, 1), q(0, 1)), (q(0, 1), q(0, 1)), (q(1, 1), q(-1, 1))],
    )
    .expect("a");
    let b = MatrixValue::from_complex_exact_row_major(
        2,
        2,
        vec![(q(1, 1), q(0, 1)), (q(0, 1), q(1, 1)), (q(0, 1), q(-1, 1)), (q(1, 1), q(0, 1))],
    )
    .expect("b");
    let m = matmul(&a, &b).expect("matmul");
    assert_eq!(
        m.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(1, 1),
            im: q(1, 1),
        }
    );
    assert_eq!(
        m.get(0, 1).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(-1, 1),
            im: q(1, 1),
        }
    );
    let h = hadamard(&a, &a).expect("hadamard");
    // (1+i)^2 = 1+2i-1 = 2i
    assert_eq!(
        h.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(0, 1),
            im: q(2, 1),
        }
    );
}

#[test]
fn l0_complex_exact_elementwise_divide() {
    let a = MatrixValue::from_complex_exact_row_major(1, 1, vec![(q(1, 1), q(1, 1))]).expect("a");
    let b = MatrixValue::from_complex_exact_row_major(1, 1, vec![(q(1, 1), q(0, 1))]).expect("b");
    let d = elementwise_divide(&a, &b).expect("div");
    assert_eq!(
        d.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(1, 1),
            im: q(1, 1),
        }
    );
}

#[test]
fn l0_complex_exact_tril_triu() {
    let m = MatrixValue::from_complex_exact_row_major(
        2,
        2,
        vec![(q(1, 1), q(1, 1)), (q(2, 1), q(0, 1)), (q(3, 1), q(0, 1)), (q(4, 1), q(-1, 1))],
    )
    .expect("m");
    let lower = tril(&m).expect("tril");
    assert_eq!(
        lower.get(0, 1).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(0, 1),
            im: q(0, 1),
        }
    );
    assert_eq!(
        lower.get(1, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(3, 1),
            im: q(0, 1),
        }
    );
    let upper = triu(&m).expect("triu");
    assert_eq!(
        upper.get(1, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(0, 1),
            im: q(0, 1),
        }
    );
    assert_eq!(
        upper.get(0, 1).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(2, 1),
            im: q(0, 1),
        }
    );
}

#[test]
fn l0_complex_exact_flatten_row_major() {
    let m = MatrixValue::from_complex_exact_row_major(
        2,
        2,
        vec![(q(1, 1), q(1, 1)), (q(2, 1), q(0, 1)), (q(3, 1), q(0, 1)), (q(4, 1), q(0, 1))],
    )
    .expect("m");
    let flat = flatten_row_major(&m).expect("flatten");
    assert_eq!(flat.shape(), MatrixShape::new(1, 4));
    assert_eq!(
        flat.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(1, 1),
            im: q(1, 1),
        }
    );
    assert_eq!(
        flat.get(0, 2).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(3, 1),
            im: q(0, 1),
        }
    );
}

#[test]
fn l0_complex_exact_reverse_join_slice() {
    let m = MatrixValue::from_complex_exact_row_major(
        2,
        2,
        vec![(q(1, 1), q(1, 1)), (q(2, 1), q(0, 1)), (q(3, 1), q(0, 1)), (q(4, 1), q(0, 1))],
    )
    .expect("m");
    let rev = reverse_matrix(&m).expect("reverse");
    assert_eq!(
        rev.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(3, 1),
            im: q(0, 1),
        }
    );
    let a = MatrixValue::from_complex_exact_row_major(1, 1, vec![(q(1, 1), q(1, 1))]).expect("a");
    let b = MatrixValue::from_complex_exact_row_major(1, 1, vec![(q(2, 1), q(0, 1))]).expect("b");
    let joined = join_matrices(&[&a, &b]).expect("join");
    assert_eq!(joined.shape(), MatrixShape::new(1, 2));
    let sliced = slice_matrix(&m, &IndexSpec::Scalar { row: 0, col: 1 }).expect("slice");
    assert_eq!(
        sliced.get(0, 0).unwrap(),
        MatrixEntry::ComplexExact {
            re: q(2, 1),
            im: q(0, 1),
        }
    );
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
    assert_eq!(v.value.get(0, 0).unwrap(), MatrixEntry::Integer(i(30)));
    assert_eq!(v.shape.rows, 1);
    assert_eq!(v.shape.cols, 1);
    assert_eq!(v.guarantee, AlgorithmGuarantee::Exact);
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
            &x.value,
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
fn l1_machine_solve_singular_rank_deficient() {
    let a = MatrixValue::from_f64_row_major(2, 2, vec![1.0, 2.0, 2.0, 4.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(2, 1, vec![1.0, 0.0]).unwrap();
    let sol = solve_machine(&a, &b, 1e-12).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Singular);
    assert!(sol.solution.is_none());
    let w = sol.witness.expect("singular carries witness");
    assert!(w.residual_inf.is_none());
    assert_eq!(w.numerical_rank, 1);
    assert!(sol.solution.is_none());
    assert_eq!(sol.guarantee, AlgorithmGuarantee::Approximate);
}

#[test]
fn l1_machine_solve_with_residual() {
    let a = MatrixValue::from_f64_row_major(2, 2, vec![3.0, 1.0, 1.0, 2.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(2, 1, vec![9.0, 8.0]).unwrap();
    let sol = solve_machine(&a, &b, 1e-12).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Unique);
    let w = sol.witness.unwrap();
    assert!(w.residual_inf.expect("unique carries residual") < 1e-9);
    assert_eq!(w.numerical_rank, 2);
    let conditioning = sol.solution.as_ref().and_then(|s| s.conditioning).expect("unique carries conditioning");
    assert!(conditioning.is_finite() && conditioning >= 1.0);
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
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Transpose { matrix: matrix.into() })));
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
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
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
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
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
    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let b = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(1)]).unwrap());
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
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Rank { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("rank goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
}

#[test]
fn goal_machine_solve_singular_projects_residual() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
        runtime::values::arena::application_display_name,
    };
    use athena_ir::Atom;
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let a = session.matrix_objects.intern(MatrixValue::from_f64_row_major(2, 2, vec![1.0, 2.0, 2.0, 4.0]).unwrap());
    let b = session.matrix_objects.intern(MatrixValue::from_f64_row_major(2, 1, vec![1.0, 0.0]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Solve { a: a.into(), b: b.into() })));
    let result_id = match execute_ir_request(&mut session, request) {
        Ok(id) => id,
        Err(err) => panic!("machine singular goal failed: {err}"),
    };
    let result = session.results.get(result_id).expect("result");
    assert_eq!(result.status, ComputationStatus::Partial);
    let term = result.symbolic_term.expect("singular residual");
    assert_eq!(application_display_name(&session, term).as_deref(), Some("LinearSolve"));
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Application { arguments, .. }) if arguments.len() == 1 => {
            assert!(matches!(session.arena.get(arguments[0]), Some(athena_ir::TermNode::Atom(Atom::Symbol(_)))));
        }
        other => panic!("expected LinearSolve[Singular], got {other:?}"),
    }
}

#[test]
fn goal_exact_solve_inconsistent_projects_disposition_residual() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
        runtime::values::arena::application_display_name,
    };
    use athena_ir::Atom;
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(0)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Solve { a: a.into(), b: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("exact inconsistent goal");
    let result = session.results.get(result_id).expect("result");
    // Exact domain proves inconsistency; projection is a disposition residual, not an empty list.
    assert_eq!(result.status, ComputationStatus::Exact);
    let term = result.symbolic_term.expect("inconsistent residual");
    assert_eq!(application_display_name(&session, term).as_deref(), Some("LinearSolve"));
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Application { arguments, .. }) if arguments.len() == 1 => {
            assert!(matches!(session.arena.get(arguments[0]), Some(athena_ir::TermNode::Atom(Atom::Symbol(_)))));
        }
        other => panic!("expected LinearSolve[Inconsistent], got {other:?}"),
    }
}


#[test]
fn goal_exact_solve_infinite_projects_disposition_residual() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
        runtime::values::arena::application_display_name,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let a = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let b = session
        .matrix_objects
        .intern(MatrixValue::from_integers_row_major(2, 1, vec![i(2), i(4)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Solve { a: a.into(), b: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("exact infinite goal");
    let result = session.results.get(result_id).expect("result");
    // Exact domain certifies an affine family; coverage stays Partial because free vars remain.
    assert_eq!(result.status, ComputationStatus::Exact);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
    let term = result.symbolic_term.expect("infinite residual");
    assert_eq!(application_display_name(&session, term).as_deref(), Some("LinearSolve"));
    match session.arena.get(term) {
        Some(TermNode::Application { arguments, .. }) if arguments.len() >= 1 => {
            assert!(matches!(session.arena.get(arguments[0]), Some(TermNode::Atom(Atom::Symbol(_)))));
        }
        other => panic!("expected LinearSolve[Infinite, …], got {other:?}"),
    }
}


#[test]
fn goal_rref_projects_nested_list_via_execution() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
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
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(0), i(1)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Inverse { matrix: matrix.into() })));
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
fn l1_invert_exact_singular_disposition() {
    use athena_engine::domains::linear_algebra::invert_exact;

    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap();
    let inv = invert_exact(&a).unwrap();
    assert_eq!(inv.disposition, SolveDisposition::Singular);
    assert!(inv.inverse.is_none());
}

#[test]
fn goal_inverse_singular_projects_residual() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
        runtime::values::arena::application_display_name,
    };
    use athena_ir::Atom;
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Inverse { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("singular inverse goal");
    let result = session.results.get(result_id).expect("result");
    assert_eq!(result.status, ComputationStatus::Partial);
    let term = result.symbolic_term.expect("singular residual");
    assert_eq!(application_display_name(&session, term).as_deref(), Some("Inverse"));
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Application { arguments, .. }) if arguments.len() == 1 => {
            assert!(matches!(session.arena.get(arguments[0]), Some(athena_ir::TermNode::Atom(Atom::Symbol(_)))));
        }
        other => panic!("expected Inverse[Singular], got {other:?}"),
    }
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            athena_engine::runtime::results::ResultEvidence::TrustedKernelSummary { summary, .. }
                if summary.contains("disposition=Singular")
        )),
        "Inverse Singular must publish disposition, got {:?}",
        result.evidence
    );
}

#[test]
fn goal_trace_projects_integer_via_execution() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Trace { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("trace goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)));
}

#[test]
fn goal_dot_matrix_vector_projects_flat_list() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    // Explicit column vector — dialects must choose orientation, kernel does not guess from `1×n`.
    let b = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 1, vec![i(1), i(1)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Dot { lhs: a.into(), rhs: b.into() })));
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
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let b = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 2, vec![i(1), i(1)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Dot { lhs: a.into(), rhs: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("shape errors become results");
    let result = session.results.get(result_id).expect("result");
    assert_ne!(result.status, ComputationStatus::Exact, "incompatible 2x2·1x2 must not succeed as Exact");
    assert!(
        result.diagnostics.iter().any(|d| d.code == athena_types::DiagnosticCode::ShapeMismatch) || result.symbolic_term.is_none(),
        "expected ShapeMismatch diagnostic or no successful projection, got status={:?} diags={:?}",
        result.status,
        result.diagnostics
    );
}

#[test]
fn goal_cross_projects_flat_list() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 3, vec![i(1), i(0), i(0)]).unwrap());
    let b = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 3, vec![i(0), i(1), i(0)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Cross { lhs: a.into(), rhs: b.into() })));
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
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::NullSpace {
        matrix: matrix.into(),
        column_basis: false,
    })));
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
    let evidence = &session.results.get(result_id).expect("result").evidence;
    assert!(
        evidence.iter().any(|e| matches!(
            e,
            athena_engine::runtime::results::ResultEvidence::TrustedKernelSummary { summary, .. }
                if summary.contains("nullity=1")
        )),
        "NullSpace must publish nullity evidence, got {evidence:?}"
    );
}

#[test]
fn goal_nullspace_column_basis_projects_column() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::NullSpace {
        matrix: matrix.into(),
        column_basis: true,
    })));
    let result_id = execute_ir_request(&mut session, request).expect("nullspace column goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    // Column basis of one free vector → 2×1 → nested [[-2],[1]] or similar.
    assert_eq!(rows.len(), 2);
    for (idx, expected) in [(0, -2), (1, 1)] {
        let TermNode::Collection { elements: cell, .. } = session.arena.get(rows[idx]).expect("cell")
        else {
            panic!("expected singleton row");
        };
        assert_eq!(cell.len(), 1);
        assert!(matches!(session.arena.get(cell[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(expected)));
    }
}

#[test]
fn goal_nullspace_column_basis_full_rank_empty() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::TermNode;

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(0), i(1)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::NullSpace {
        matrix: matrix.into(),
        column_basis: true,
    })));
    let result_id = execute_ir_request(&mut session, request).expect("nullspace empty");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    // n×0 projects as empty list of rows.
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("rows")
    else {
        panic!("expected nested list");
    };
    assert!(rows.is_empty());
}

#[test]
fn goal_norm_projects_integer() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 2, vec![i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Norm { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("norm goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    assert!(matches!(session.arena.get(term), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(5)));
}

#[test]
fn l1_condition_number_well_conditioned() {
    use athena_engine::domains::linear_algebra::condition_number_machine;

    let a = MatrixValue::from_f64_row_major(2, 2, vec![2.0, 0.0, 0.0, 2.0]).unwrap();
    let est = condition_number_machine(&a, 1e-12).unwrap();
    assert!(est.value.is_finite() && est.value >= 1.0 && est.value < 1.0 + 1e-9);
    assert_eq!(est.numerical_rank, 2);
    assert_eq!(est.guarantee, AlgorithmGuarantee::Approximate);
}

#[test]
fn l1_condition_number_singular_is_inf() {
    use athena_engine::domains::linear_algebra::condition_number_machine;

    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(2), i(4)]).unwrap();
    let est = condition_number_machine(&a, 1e-12).unwrap();
    assert!(est.value.is_infinite());
    assert_eq!(est.numerical_rank, 1);
}

#[test]
fn goal_condition_number_projects_machine_float() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_f64_row_major(2, 2, vec![2.0, 0.0, 0.0, 2.0]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::ConditionNumber {
        matrix: matrix.into(),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("cond goal");
    let result = session.results.get(result_id).expect("result");
    assert_eq!(result.status, ComputationStatus::Approximate);
    let term = result.symbolic_term.expect("projected");
    match session.arena.get(term) {
        Some(TermNode::Atom(Atom::Number(n))) => {
            let v = n.as_machine_f64().expect("machine float");
            assert!((v - 1.0).abs() < 1e-9);
        }
        other => panic!("expected machine number, got {other:?}"),
    }
}

#[test]
fn l1_right_solve_exact_row_vector() {
    // MATLAB `[1, 2] / [[1, 2], [3, 4]]` → `[1, 0]`
    let a = MatrixValue::from_integers_row_major(1, 2, vec![i(1), i(2)]).unwrap();
    let b = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let sol = right_solve_exact(&a, &b).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Unique);
    let x = sol.particular.expect("particular");
    assert_eq!(x.shape.rows, 1);
    assert_eq!(x.shape.cols, 2);
    assert_eq!(x.value.get(0, 0).unwrap(), MatrixEntry::Rational(q(1, 1)));
    assert_eq!(x.value.get(0, 1).unwrap(), MatrixEntry::Rational(q(0, 1)));
}

#[test]
fn l1_right_solve_exact_square_identity() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let sol = right_solve_exact(&a, &a).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Unique);
    let x = sol.particular.expect("particular");
    assert_eq!(x.value.get(0, 0).unwrap(), MatrixEntry::Rational(q(1, 1)));
    assert_eq!(x.value.get(0, 1).unwrap(), MatrixEntry::Rational(q(0, 1)));
    assert_eq!(x.value.get(1, 0).unwrap(), MatrixEntry::Rational(q(0, 1)));
    assert_eq!(x.value.get(1, 1).unwrap(), MatrixEntry::Rational(q(1, 1)));
}

#[test]
fn l1_right_solve_machine_with_residual() {
    use athena_engine::domains::linear_algebra::right_solve_machine;

    let a = MatrixValue::from_f64_row_major(1, 2, vec![1.0, 2.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(2, 2, vec![1.0, 2.0, 3.0, 4.0]).unwrap();
    let sol = right_solve_machine(&a, &b, 1e-12).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Unique);
    let w = sol.witness.expect("witness");
    assert!(w.residual_inf.expect("residual") < 1e-9);
    let x = sol.solution.expect("solution");
    assert!(
        (match x.value.get(0, 0).unwrap() {
            MatrixEntry::MachineF64(v) => v,
            _ => panic!("f64"),
        } - 1.0)
            .abs()
            < 1e-9
    );
}

#[test]
fn l1_right_solve_machine_singular() {
    use athena_engine::domains::linear_algebra::right_solve_machine;

    let a = MatrixValue::from_f64_row_major(1, 2, vec![1.0, 0.0]).unwrap();
    let b = MatrixValue::from_f64_row_major(2, 2, vec![1.0, 2.0, 2.0, 4.0]).unwrap();
    let sol = right_solve_machine(&a, &b, 1e-12).unwrap();
    assert_eq!(sol.disposition, SolveDisposition::Singular);
    assert!(sol.solution.is_none());
    assert!(sol.witness.expect("witness").residual_inf.is_none());
}

#[test]
fn goal_right_solve_projects_row() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};

    let mut session = Session::new();
    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 2, vec![i(1), i(2)]).unwrap());
    let b = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::RightSolve { a: a.into(), b: b.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("right solve goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("projected");
    let TermNode::Collection { elements: items, .. } = session.arena.get(term).expect("row")
    else {
        panic!("expected flat or nested list");
    };
    // 1×2 may project as flat list or nested single row.
    if items.len() == 2 {
        assert!(matches!(session.arena.get(items[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
        assert!(matches!(session.arena.get(items[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    }
    else {
        assert_eq!(items.len(), 1);
        let TermNode::Collection { elements: r0, .. } = session.arena.get(items[0]).expect("r0")
        else {
            panic!("row0");
        };
        assert_eq!(r0.len(), 2);
        assert!(matches!(session.arena.get(r0[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(1)));
        assert!(matches!(session.arena.get(r0[1]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(0)));
    }
}

#[test]
fn l0_tril_triu_mask_integer() {
    let m = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let lo = tril(&m).unwrap();
    assert_eq!(lo.get(0, 0).unwrap(), MatrixEntry::Integer(i(1)));
    assert_eq!(lo.get(0, 1).unwrap(), MatrixEntry::Integer(i(0)));
    assert_eq!(lo.get(1, 0).unwrap(), MatrixEntry::Integer(i(3)));
    assert_eq!(lo.get(1, 1).unwrap(), MatrixEntry::Integer(i(4)));
    let up = triu(&m).unwrap();
    assert_eq!(up.get(0, 0).unwrap(), MatrixEntry::Integer(i(1)));
    assert_eq!(up.get(0, 1).unwrap(), MatrixEntry::Integer(i(2)));
    assert_eq!(up.get(1, 0).unwrap(), MatrixEntry::Integer(i(0)));
    assert_eq!(up.get(1, 1).unwrap(), MatrixEntry::Integer(i(4)));
}

#[test]
fn l0_kronecker_row_vectors() {
    // MATLAB kron([1, 2], [3, 4]) → [3, 4, 6, 8]
    let a = MatrixValue::from_integers_row_major(1, 2, vec![i(1), i(2)]).unwrap();
    let b = MatrixValue::from_integers_row_major(1, 2, vec![i(3), i(4)]).unwrap();
    let k = kronecker(&a, &b).unwrap();
    assert_eq!(k.shape().rows, 1);
    assert_eq!(k.shape().cols, 4);
    assert_eq!(k.get(0, 0).unwrap(), MatrixEntry::Integer(i(3)));
    assert_eq!(k.get(0, 1).unwrap(), MatrixEntry::Integer(i(4)));
    assert_eq!(k.get(0, 2).unwrap(), MatrixEntry::Integer(i(6)));
    assert_eq!(k.get(0, 3).unwrap(), MatrixEntry::Integer(i(8)));
}

#[test]
fn goal_tril_and_kronecker_project_nested_list() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Tril { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("tril goal");
    let result = session.results.get(result_id).expect("result");
    assert_eq!(result.status, ComputationStatus::Exact);
    let term = result.symbolic_term.expect("projected");
    let TermNode::Collection { elements: rows, .. } = session.arena.get(term).expect("list")
    else {
        panic!("expected nested list");
    };
    assert_eq!(rows.len(), 2);

    let a = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 2, vec![i(1), i(2)]).unwrap());
    let b = session.matrix_objects.intern(MatrixValue::from_integers_row_major(1, 2, vec![i(3), i(4)]).unwrap());
    let request = AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::Kronecker {
        lhs: a.into(),
        rhs: b.into(),
    })));
    let result_id = execute_ir_request(&mut session, request).expect("kronecker goal");
    let result = session.results.get(result_id).expect("result");
    assert_eq!(result.status, ComputationStatus::Exact);
    let term = result.symbolic_term.expect("projected");
    // 1×4 may project as flat List or nested single row.
    let TermNode::Collection { elements: items, .. } = session.arena.get(term).expect("list")
    else {
        panic!("expected list");
    };
    let flat: Vec<_> = if items.len() == 4 {
        items.to_vec()
    }
    else if items.len() == 1 {
        let TermNode::Collection { elements: row, .. } = session.arena.get(items[0]).expect("row")
        else {
            panic!("expected nested row");
        };
        assert_eq!(row.len(), 4);
        row.to_vec()
    }
    else {
        panic!("unexpected projection len {}", items.len());
    };
    assert!(matches!(session.arena.get(flat[0]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(3)));
    assert!(matches!(session.arena.get(flat[3]), Some(TermNode::Atom(Atom::Number(n))) if n.as_exact_integer() == Some(8)));
}

#[test]
fn l0_matrix_structure_predicates() {
    let eye = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(0), i(1)]).unwrap();
    assert!(is_diagonal(&eye).unwrap());
    assert!(is_lower_triangular(&eye).unwrap());
    assert!(is_symmetric(&eye).unwrap());
    let lower = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(3), i(4)]).unwrap();
    assert!(!is_diagonal(&lower).unwrap());
    assert!(is_lower_triangular(&lower).unwrap());
    assert!(!is_symmetric(&lower).unwrap());
    let full = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    assert!(!is_lower_triangular(&full).unwrap());
}

#[test]
fn goal_is_diagonal_projects_scalar_one() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        execution::execute_ir_request,
    };
    use athena_ir::{Atom, TermNode};
    use athena_types::ComputationStatus;

    let mut session = Session::new();
    let matrix = session.matrix_objects.intern(MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(0), i(0), i(2)]).unwrap());
    let request =
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::LinearAlgebra(LinearAlgebraRequest::IsDiagonal { matrix: matrix.into() })));
    let result_id = execute_ir_request(&mut session, request).expect("isdiag goal");
    let result = session.results.get(result_id).expect("result");
    assert_eq!(result.status, ComputationStatus::Exact);
    let term = result.symbolic_term.expect("projected");
    match session.arena.get(term) {
        Some(TermNode::Atom(Atom::Number(n))) => assert_eq!(n.as_exact_integer(), Some(1)),
        other => panic!("expected scalar 1, got {other:?}"),
    }
}
