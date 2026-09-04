//! 矩阵不变量、精确/机器双路径与跨方言规范奇偶校验。

use athena_engine::{
    AlgorithmGuarantee, DialectArgs, DialectMatrixOp, DialectOrigin, DomainRequest, DomainResult, IndexSpec,
    LinearAlgebraRequest, LinearAlgebraResult, LinearAlgebraValue, MatrixEntry, MatrixEqualityKind, MatrixParent, MatrixShape,
    MatrixValue, Session, SolveDisposition, StorageOrder, det_bareiss, execute_domain, execute_linear_algebra, hadamard,
    lower_1based_scalar, lower_dialect_op, matlab_star_kind, matmul, matrices_equal, rank_exact, solve_exact, solve_machine,
    transpose,
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
    let mut owned = m.clone();
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
fn dialect_canonical_parity_matmul_vs_hadamard() {
    let a = MatrixValue::from_integers_row_major(2, 2, vec![i(1), i(2), i(3), i(4)]).unwrap();
    let b = MatrixValue::from_integers_row_major(2, 2, vec![i(5), i(6), i(7), i(8)]).unwrap();

    let mm_mathematica = lower_dialect_op(
        DialectOrigin::Mathematica,
        DialectMatrixOp::MatMul,
        DialectArgs::Binary { lhs: a.clone(), rhs: b.clone() },
    )
    .unwrap();
    let mm_matlab = lower_dialect_op(
        DialectOrigin::Matlab,
        matlab_star_kind(false),
        DialectArgs::Binary { lhs: a.clone(), rhs: b.clone() },
    )
    .unwrap();
    assert_eq!(mm_mathematica, mm_matlab);

    let had_mathematica = lower_dialect_op(
        DialectOrigin::Mathematica,
        DialectMatrixOp::Hadamard,
        DialectArgs::Binary { lhs: a.clone(), rhs: b.clone() },
    )
    .unwrap();
    let had_matlab =
        lower_dialect_op(DialectOrigin::Matlab, matlab_star_kind(true), DialectArgs::Binary { lhs: a.clone(), rhs: b.clone() })
            .unwrap();
    assert_eq!(had_mathematica, had_matlab);
    assert_ne!(mm_matlab, had_matlab);

    let r1 = execute_linear_algebra(mm_mathematica);
    let r2 = execute_linear_algebra(mm_matlab);
    assert_eq!(r1, r2);
}

#[test]
fn dialect_1based_index_parity() {
    let m = MatrixValue::from_integers_row_major(2, 2, vec![i(10), i(20), i(30), i(40)]).unwrap();
    let spec_mma = lower_1based_scalar(DialectOrigin::Mathematica, 2, 1).unwrap();
    let spec_matlab = lower_1based_scalar(DialectOrigin::Matlab, 2, 1).unwrap();
    assert_eq!(spec_mma, spec_matlab);
    assert_eq!(spec_mma, IndexSpec::Scalar { row: 1, col: 0 });

    let req_mma = lower_dialect_op(
        DialectOrigin::Mathematica,
        DialectMatrixOp::IndexScalar,
        DialectArgs::Index { matrix: m.clone(), row_1based: 2, col_1based: 1 },
    )
    .unwrap();
    let req_matlab = lower_dialect_op(
        DialectOrigin::Matlab,
        DialectMatrixOp::IndexScalar,
        DialectArgs::Index { matrix: m, row_1based: 2, col_1based: 1 },
    )
    .unwrap();
    assert_eq!(req_mma, req_matlab);
    let LinearAlgebraResult::Ok { value: LinearAlgebraValue::Matrix(v) } = execute_linear_algebra(req_mma)
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
    let a = MatrixValue::from_integers_row_major(1, 1, vec![i(7)]).unwrap();
    let req = DomainRequest::LinearAlgebra(LinearAlgebraRequest::Det { matrix: a });
    let mut session = Session::new();
    let DomainResult::LinearAlgebra(LinearAlgebraResult::Ok { value: LinearAlgebraValue::ExactDet(d) }) =
        execute_domain(&mut session, req).unwrap()
    else {
        panic!("expected exact det");
    };
    assert_eq!(d.det, q(7, 1));
}
