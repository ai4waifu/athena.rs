//! 矩阵算子：索引、切片、转置、矩阵乘、逐元素乘。

use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    index::IndexSpec,
    parent::ElementParentKind,
    shape::MatrixShape,
    value::{MatrixEntry, MatrixValue},
};
use crate::numeric_clone::{resize_integers, resize_rationals};

/// 标量索引（返回 1×1 矩阵以保持矩阵对象模型）。
pub fn index_scalar(matrix: &MatrixValue, row: u64, col: u64) -> Result<MatrixValue, Diagnostic> {
    IndexSpec::check_scalar(matrix.shape(), row, col)?;
    let entry = matrix.get(row, col)?;
    match entry {
        MatrixEntry::Integer(x) => MatrixValue::from_integers_row_major(1, 1, vec![x]),
        MatrixEntry::Rational(x) => MatrixValue::from_rationals_row_major(1, 1, vec![x]),
        MatrixEntry::MachineF64(x) => MatrixValue::from_f64_row_major(1, 1, vec![x]),
    }
}

/// 按 [`IndexSpec`] 切片并物化为自有行主序矩阵。
pub fn slice_matrix(matrix: &MatrixValue, spec: &IndexSpec) -> Result<MatrixValue, Diagnostic> {
    match spec {
        IndexSpec::Scalar { row, col } => index_scalar(matrix, *row, *col),
        IndexSpec::Slice { rows, cols } => {
            let row_ix = rows.resolve(matrix.shape().rows)?;
            let col_ix = cols.resolve(matrix.shape().cols)?;
            let out_rows = row_ix.len() as u64;
            let out_cols = col_ix.len() as u64;
            match matrix.parent().element {
                ElementParentKind::Integers => {
                    let mut data = Vec::with_capacity((out_rows * out_cols) as usize);
                    for &r in &row_ix {
                        for &c in &col_ix {
                            match matrix.get(r, c)? {
                                MatrixEntry::Integer(x) => data.push(x),
                                _ => unreachable!(),
                            }
                        }
                    }
                    MatrixValue::from_integers_row_major(out_rows, out_cols, data)
                }
                ElementParentKind::Rationals => {
                    let mut data = Vec::with_capacity((out_rows * out_cols) as usize);
                    for &r in &row_ix {
                        for &c in &col_ix {
                            match matrix.get(r, c)? {
                                MatrixEntry::Rational(x) => data.push(x),
                                _ => unreachable!(),
                            }
                        }
                    }
                    MatrixValue::from_rationals_row_major(out_rows, out_cols, data)
                }
                ElementParentKind::MachineReal => {
                    let mut data = Vec::with_capacity((out_rows * out_cols) as usize);
                    for &r in &row_ix {
                        for &c in &col_ix {
                            match matrix.get(r, c)? {
                                MatrixEntry::MachineF64(x) => data.push(x),
                                _ => unreachable!(),
                            }
                        }
                    }
                    MatrixValue::from_f64_row_major(out_rows, out_cols, data)
                }
            }
        }
    }
}

/// 转置（视图；共享缓冲）。
pub fn transpose(matrix: &MatrixValue) -> MatrixValue {
    matrix.transpose_view()
}

fn require_same_element_parent(a: &MatrixValue, b: &MatrixValue) -> Result<(), Diagnostic> {
    if a.parent().element != b.parent().element {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch)
            .detail("reason", "element_parent_mismatch")
            .detail("lhs", format!("{:?}", a.parent().element))
            .detail("rhs", format!("{:?}", b.parent().element)));
    }
    Ok(())
}

/// 矩阵乘（先做 checked shape inference）。
pub fn matmul(lhs: &MatrixValue, rhs: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    require_same_element_parent(lhs, rhs)?;
    let out_shape = lhs.shape().matmul(rhs.shape())?;
    match lhs.parent().element {
        ElementParentKind::Integers => {
            let mut data = { let mut __v = Vec::new(); resize_integers(&mut __v, out_shape.element_count()?, &Integer::zero()); __v };
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let mut acc = Integer::zero();
                    for k in 0..lhs.shape().cols {
                        let a = match lhs.get(i, k)? {
                            MatrixEntry::Integer(x) => x,
                            _ => unreachable!(),
                        };
                        let b = match rhs.get(k, j)? {
                            MatrixEntry::Integer(x) => x,
                            _ => unreachable!(),
                        };
                        acc = acc.add(&a.mul(&b));
                    }
                    data[(i * out_shape.cols + j) as usize] = acc;
                }
            }
            MatrixValue::from_integers_row_major(out_shape.rows, out_shape.cols, data)
        }
        ElementParentKind::Rationals => {
            let mut data = { let mut __v = Vec::new(); resize_rationals(&mut __v, out_shape.element_count()?, &Rational::zero()); __v };
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let mut acc = Rational::zero();
                    for k in 0..lhs.shape().cols {
                        let a = match lhs.get(i, k)? {
                            MatrixEntry::Rational(x) => x,
                            _ => unreachable!(),
                        };
                        let b = match rhs.get(k, j)? {
                            MatrixEntry::Rational(x) => x,
                            _ => unreachable!(),
                        };
                        acc = acc.add(&a.mul(&b));
                    }
                    data[(i * out_shape.cols + j) as usize] = acc;
                }
            }
            MatrixValue::from_rationals_row_major(out_shape.rows, out_shape.cols, data)
        }
        ElementParentKind::MachineReal => {
            let mut data = vec![0.0; out_shape.element_count()?];
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let mut acc = 0.0;
                    for k in 0..lhs.shape().cols {
                        let a = match lhs.get(i, k)? {
                            MatrixEntry::MachineF64(x) => x,
                            _ => unreachable!(),
                        };
                        let b = match rhs.get(k, j)? {
                            MatrixEntry::MachineF64(x) => x,
                            _ => unreachable!(),
                        };
                        acc += a * b;
                    }
                    data[(i * out_shape.cols + j) as usize] = acc;
                }
            }
            MatrixValue::from_f64_row_major(out_shape.rows, out_shape.cols, data)
        }
    }
}

/// 逐元素乘（Hadamard）；shape 必须一致。
pub fn hadamard(lhs: &MatrixValue, rhs: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    require_same_element_parent(lhs, rhs)?;
    let out_shape = MatrixShape::hadamard(lhs.shape(), rhs.shape())?;
    match lhs.parent().element {
        ElementParentKind::Integers => {
            let mut data = Vec::with_capacity(out_shape.element_count()?);
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let a = match lhs.get(i, j)? {
                        MatrixEntry::Integer(x) => x,
                        _ => unreachable!(),
                    };
                    let b = match rhs.get(i, j)? {
                        MatrixEntry::Integer(x) => x,
                        _ => unreachable!(),
                    };
                    data.push(a.mul(&b));
                }
            }
            MatrixValue::from_integers_row_major(out_shape.rows, out_shape.cols, data)
        }
        ElementParentKind::Rationals => {
            let mut data = Vec::with_capacity(out_shape.element_count()?);
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let a = match lhs.get(i, j)? {
                        MatrixEntry::Rational(x) => x,
                        _ => unreachable!(),
                    };
                    let b = match rhs.get(i, j)? {
                        MatrixEntry::Rational(x) => x,
                        _ => unreachable!(),
                    };
                    data.push(a.mul(&b));
                }
            }
            MatrixValue::from_rationals_row_major(out_shape.rows, out_shape.cols, data)
        }
        ElementParentKind::MachineReal => {
            let mut data = Vec::with_capacity(out_shape.element_count()?);
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let a = match lhs.get(i, j)? {
                        MatrixEntry::MachineF64(x) => x,
                        _ => unreachable!(),
                    };
                    let b = match rhs.get(i, j)? {
                        MatrixEntry::MachineF64(x) => x,
                        _ => unreachable!(),
                    };
                    data.push(a * b);
                }
            }
            MatrixValue::from_f64_row_major(out_shape.rows, out_shape.cols, data)
        }
    }
}
