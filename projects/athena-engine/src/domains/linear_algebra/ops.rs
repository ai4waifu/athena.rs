//! 矩阵算子：索引、切片、转置、矩阵乘、逐元素乘 / 除 / 幂。

use athena_numeric::{Integer, Rational};
use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    index::IndexSpec,
    parent::ElementParentKind,
    shape::MatrixShape,
    value::{MatrixEntry, MatrixValue},
};
use crate::runtime::values::numeric_clone::{resize_integers, resize_rationals};

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
            let mut data = {
                let mut __v = Vec::new();
                resize_integers(&mut __v, out_shape.element_count()?, &Integer::zero());
                __v
            };
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
            let mut data = {
                let mut __v = Vec::new();
                resize_rationals(&mut __v, out_shape.element_count()?, &Rational::zero());
                __v
            };
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

/// 逐元素除；shape 必须一致。整数父域提升为精确有理。
pub fn elementwise_divide(lhs: &MatrixValue, rhs: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    require_same_element_parent(lhs, rhs)?;
    let out_shape = MatrixShape::hadamard(lhs.shape(), rhs.shape())?;
    match lhs.parent().element {
        ElementParentKind::Integers => {
            let mut data = Vec::with_capacity(out_shape.element_count()?);
            for i in 0..out_shape.rows {
                for j in 0..out_shape.cols {
                    let a = match lhs.get(i, j)? {
                        MatrixEntry::Integer(x) => Rational::from_integer(x),
                        _ => unreachable!(),
                    };
                    let b = match rhs.get(i, j)? {
                        MatrixEntry::Integer(x) => Rational::from_integer(x),
                        _ => unreachable!(),
                    };
                    data.push(a.try_div(&b)?);
                }
            }
            MatrixValue::from_rationals_row_major(out_shape.rows, out_shape.cols, data)
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
                    data.push(a.try_div(&b)?);
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
                    data.push(a / b);
                }
            }
            MatrixValue::from_f64_row_major(out_shape.rows, out_shape.cols, data)
        }
    }
}

/// 逐元素幂；shape 必须一致。
///
/// Exact parents require a non-negative integer exponent that fits `u32`. Machine real uses `powf`.
pub fn elementwise_power(lhs: &MatrixValue, rhs: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
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
                    let powered = a.pow(&b).map_err(|_| {
                        Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "elementwise_power_integer")
                    })?;
                    data.push(powered);
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
                    if !b.is_integer() {
                        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "elementwise_power_exp_not_int"));
                    }
                    let Some(exp_i) = b.numerator().to_i64()
                    else {
                        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "elementwise_power_exp_too_large"));
                    };
                    if exp_i < 0 {
                        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "elementwise_power_neg_exp"));
                    }
                    if exp_i > u32::MAX as i64 {
                        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "elementwise_power_exp_too_large"));
                    }
                    data.push(a.pow_u32(exp_i as u32)?);
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
                    data.push(a.powf(b));
                }
            }
            MatrixValue::from_f64_row_major(out_shape.rows, out_shape.cols, data)
        }
    }
}

/// 行主序展平为 `1×(rows*cols)`（Mathematica `Flatten` 对矩形嵌套 List 的默认序）。
pub fn flatten_row_major(matrix: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let n = matrix.shape().element_count()?;
    let n_u64 = n as u64;
    match matrix.parent().element {
        ElementParentKind::Integers => {
            let mut data = Vec::with_capacity(n);
            for i in 0..rows {
                for j in 0..cols {
                    match matrix.get(i, j)? {
                        MatrixEntry::Integer(x) => data.push(x),
                        _ => unreachable!(),
                    }
                }
            }
            MatrixValue::from_integers_row_major(1, n_u64, data)
        }
        ElementParentKind::Rationals => {
            let mut data = Vec::with_capacity(n);
            for i in 0..rows {
                for j in 0..cols {
                    match matrix.get(i, j)? {
                        MatrixEntry::Rational(x) => data.push(x),
                        _ => unreachable!(),
                    }
                }
            }
            MatrixValue::from_rationals_row_major(1, n_u64, data)
        }
        ElementParentKind::MachineReal => {
            let mut data = Vec::with_capacity(n);
            for i in 0..rows {
                for j in 0..cols {
                    match matrix.get(i, j)? {
                        MatrixEntry::MachineF64(x) => data.push(x),
                        _ => unreachable!(),
                    }
                }
            }
            MatrixValue::from_f64_row_major(1, n_u64, data)
        }
    }
}

/// Explicit matrix product for `Dot` operands (no shape guessing / auto-transpose).
///
/// Dialects must lower vectors with an explicit rank and orientation. A `1×n` row is not
/// silently treated as an `n×1` column.
pub fn dot(lhs: &MatrixValue, rhs: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    require_same_element_parent(lhs, rhs)?;
    matmul(lhs, rhs)
}

/// 三维叉积（`1×3` / `3×1` 向量）；结果为 `1×3` 行向量。
pub fn cross(lhs: &MatrixValue, rhs: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    require_same_element_parent(lhs, rhs)?;
    if lhs.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "cross_exact_only"));
    }
    let a = vector3_rationals(lhs)?;
    let b = vector3_rationals(rhs)?;
    let c0 = a[1].mul(&b[2]).sub(&a[2].mul(&b[1]));
    let c1 = a[2].mul(&b[0]).sub(&a[0].mul(&b[2]));
    let c2 = a[0].mul(&b[1]).sub(&a[1].mul(&b[0]));
    MatrixValue::from_rationals_row_major(1, 3, vec![c0, c1, c2])
}

fn vector3_rationals(matrix: &MatrixValue) -> Result<[Rational; 3], Diagnostic> {
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let entries = if rows == 1 && cols == 3 {
        [matrix.get(0, 0)?, matrix.get(0, 1)?, matrix.get(0, 2)?]
    }
    else if rows == 3 && cols == 1 {
        [matrix.get(0, 0)?, matrix.get(1, 0)?, matrix.get(2, 0)?]
    }
    else {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch)
            .detail("reason", "cross_requires_3_vector")
            .detail("shape", format!("{rows}x{cols}")));
    };
    let mut out = [Rational::zero(), Rational::zero(), Rational::zero()];
    for (i, entry) in entries.into_iter().enumerate() {
        out[i] = match entry {
            MatrixEntry::Integer(z) => Rational::from_integer(z),
            MatrixEntry::Rational(r) => r,
            MatrixEntry::MachineF64(_) => {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "cross_entry_machine"));
            }
        };
    }
    Ok(out)
}
