//! 机器路径：部分主元 LU、三角求解、残差。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    matrix_result::MatrixResult,
    status::{AlgorithmGuarantee, MachineSolveWitness, SolveDisposition},
    value::MatrixValue,
};

/// 机器 LU 分解（`PA = LU`，`A` 被原地覆盖为组合矩阵）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct MachineLuFactorization {
    /// 组合 `L`/`U`（单位对角 L 的严格下三角 + U）。
    pub combined: MatrixValue,
    /// 行置换（`pivots[i]` 为第 `i` 步交换行）。
    pub pivots: Vec<u64>,
    /// 数值秩估计。
    pub numerical_rank: u64,
    /// 主元阈值。
    pub pivot_threshold: f64,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

impl MachineLuFactorization {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            combined: self.combined.owning_copy(),
            pivots: self.pivots.clone(),
            numerical_rank: self.numerical_rank,
            pivot_threshold: self.pivot_threshold,
            guarantee: self.guarantee,
        }
    }
}

/// 机器求解结果。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct MachineSolveResult {
    /// 分类。
    pub disposition: SolveDisposition,
    /// 解（`n×1`，Living 16 `MatrixResult` 信封）。
    pub solution: Option<MatrixResult>,
    /// 残差见证。
    pub witness: Option<MachineSolveWitness>,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

impl MachineSolveResult {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            disposition: self.disposition.owning_copy(),
            solution: self.solution.as_ref().map(MatrixResult::owning_copy),
            witness: self.witness,
            guarantee: self.guarantee,
        }
    }
}

fn idx(cols: u64, r: u64, c: u64) -> usize {
    (r * cols + c) as usize
}

/// 部分主元 LU。
pub fn lu_partial_pivot(matrix: &MatrixValue, pivot_threshold: f64) -> Result<MachineLuFactorization, Diagnostic> {
    if !matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "lu_requires_machine"));
    }
    if !matrix.shape().is_square() {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "lu_requires_square"));
    }
    let n = matrix.shape().rows;
    let mut a = matrix.to_f64_row_major()?;
    let mut pivots = Vec::with_capacity(n as usize);
    let mut rank = 0u64;
    for k in 0..n {
        let mut piv = k;
        let mut best = a[idx(n, k, k)].abs();
        for i in (k + 1)..n {
            let v = a[idx(n, i, k)].abs();
            if v > best {
                best = v;
                piv = i;
            }
        }
        pivots.push(piv);
        if best <= pivot_threshold {
            continue;
        }
        if piv != k {
            for j in 0..n {
                let i1 = idx(n, k, j);
                let i2 = idx(n, piv, j);
                a.swap(i1, i2);
            }
        }
        let diag = a[idx(n, k, k)];
        for i in (k + 1)..n {
            a[idx(n, i, k)] /= diag;
            let lik = a[idx(n, i, k)];
            for j in (k + 1)..n {
                a[idx(n, i, j)] -= lik * a[idx(n, k, j)];
            }
        }
        rank += 1;
    }
    Ok(MachineLuFactorization {
        combined: MatrixValue::from_f64_row_major(n, n, a)?,
        pivots,
        numerical_rank: rank,
        pivot_threshold,
        guarantee: AlgorithmGuarantee::Approximate,
    })
}

fn apply_pivots(b: &mut [f64], pivots: &[u64]) {
    for (k, &piv) in pivots.iter().enumerate() {
        let k = k as u64;
        if piv != k {
            b.swap(k as usize, piv as usize);
        }
    }
}

/// 用已分解的 LU 求解 `A x = b`。
pub fn solve_lu(lu: &MachineLuFactorization, b: &MatrixValue) -> Result<MachineSolveResult, Diagnostic> {
    if !b.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "b_must_be_machine"));
    }
    let n = lu.combined.shape().rows;
    if b.shape().rows != n || b.shape().cols != 1 {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "b_shape"));
    }
    if lu.numerical_rank < n {
        return Ok(MachineSolveResult {
            disposition: SolveDisposition::Singular,
            solution: None,
            witness: Some(MachineSolveWitness {
                residual_inf: None,
                numerical_rank: lu.numerical_rank,
                pivot_threshold: lu.pivot_threshold,
            }),
            guarantee: AlgorithmGuarantee::Approximate,
        });
    }
    let mut y = b.to_f64_row_major()?;
    apply_pivots(&mut y, &lu.pivots);
    let a = lu.combined.to_f64_row_major()?;
    // 前代：Ly = Pb
    for i in 0..n {
        for j in 0..i {
            y[i as usize] -= a[idx(n, i, j)] * y[j as usize];
        }
    }
    // 回代：Ux = y
    let mut x = y;
    for i in (0..n).rev() {
        for j in (i + 1)..n {
            x[i as usize] -= a[idx(n, i, j)] * x[j as usize];
        }
        let diag = a[idx(n, i, i)];
        if diag.abs() <= lu.pivot_threshold {
            return Ok(MachineSolveResult {
                disposition: SolveDisposition::Singular,
                solution: None,
                witness: Some(MachineSolveWitness {
                    residual_inf: None,
                    numerical_rank: lu.numerical_rank,
                    pivot_threshold: lu.pivot_threshold,
                }),
                guarantee: AlgorithmGuarantee::Approximate,
            });
        }
        x[i as usize] /= diag;
    }
    let solution = MatrixResult::from_owned(MatrixValue::from_f64_row_major(n, 1, x)?, AlgorithmGuarantee::Approximate);
    Ok(MachineSolveResult {
        disposition: SolveDisposition::Unique,
        solution: Some(solution),
        witness: None,
        guarantee: AlgorithmGuarantee::Approximate,
    })
}

/// Crude κ estimate from `|U_ii|` ratio after partial-pivot LU (Living 16 conditioning witness).
pub(crate) fn conditioning_from_u_diag(lu: &MachineLuFactorization) -> Option<f64> {
    let n = lu.combined.shape().rows;
    if lu.numerical_rank < n {
        return None;
    }
    let a = lu.combined.to_f64_row_major().ok()?;
    let mut max_abs = 0.0_f64;
    let mut min_abs = f64::INFINITY;
    for i in 0..n {
        let d = a[idx(n, i, i)].abs();
        if d <= lu.pivot_threshold {
            return None;
        }
        max_abs = max_abs.max(d);
        min_abs = min_abs.min(d);
    }
    if min_abs.is_finite() && min_abs > 0.0 {
        Some(max_abs / min_abs)
    } else {
        None
    }
}

/// 机器路径求解并附残差。
pub fn solve_machine(a: &MatrixValue, b: &MatrixValue, pivot_threshold: f64) -> Result<MachineSolveResult, Diagnostic> {
    let lu = lu_partial_pivot(a, pivot_threshold)?;
    let mut result = solve_lu(&lu, b)?;
    if let Some(sol) = result.solution.take() {
        let ax = super::ops::matmul(a, &sol.value)?;
        let mut residual = 0.0_f64;
        for i in 0..b.shape().rows {
            let avi = match ax.get(i, 0)? {
                super::value::MatrixEntry::MachineF64(x) => x,
                _ => unreachable!(),
            };
            let bvi = match b.get(i, 0)? {
                super::value::MatrixEntry::MachineF64(x) => x,
                _ => unreachable!(),
            };
            residual = residual.max((avi - bvi).abs());
        }
        let conditioning = conditioning_from_u_diag(&lu);
        result.witness = Some(MachineSolveWitness {
            residual_inf: Some(residual),
            numerical_rank: lu.numerical_rank,
            pivot_threshold,
        });
        result.solution = Some(sol.with_machine_witness(residual, conditioning));
    }
    Ok(result)
}

/// 右除求解 `X B = A`（机器路径）：`Bᵀ Y = Aᵀ` 再转置。
pub fn right_solve_machine(a: &MatrixValue, b: &MatrixValue, pivot_threshold: f64) -> Result<MachineSolveResult, Diagnostic> {
    use super::ops::transpose;

    if !a.parent().element.is_machine() || !b.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "right_solve_machine_requires_machine"));
    }
    if a.shape().cols != b.shape().cols {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "right_solve_cols_mismatch"));
    }
    if !b.shape().is_square() {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "right_solve_b_requires_square"));
    }
    let at = transpose(a);
    let bt = transpose(b);
    let n = bt.shape().rows;
    let m = at.shape().cols;
    let mut y_cols: Vec<Vec<f64>> = Vec::with_capacity(m as usize);
    let mut last_witness: Option<MachineSolveWitness> = None;
    for j in 0..m {
        let mut col_data = Vec::with_capacity(n as usize);
        for i in 0..n {
            match at.get(i, j)? {
                super::value::MatrixEntry::MachineF64(x) => col_data.push(x),
                _ => {
                    return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "right_solve_at_not_machine"));
                }
            }
        }
        let rhs = MatrixValue::from_f64_row_major(n, 1, col_data)?;
        let solved = solve_machine(&bt, &rhs, pivot_threshold)?;
        match solved.disposition {
            SolveDisposition::Unique => {
                let particular = solved.solution.ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "right_solve_missing_solution")
                })?;
                y_cols.push(particular.value.to_f64_row_major()?);
                last_witness = solved.witness;
            }
            SolveDisposition::Singular => {
                return Ok(MachineSolveResult {
                    disposition: SolveDisposition::Singular,
                    solution: None,
                    witness: solved.witness,
                    guarantee: AlgorithmGuarantee::Approximate,
                });
            }
            SolveDisposition::Inconsistent => {
                return Ok(MachineSolveResult {
                    disposition: SolveDisposition::Inconsistent,
                    solution: None,
                    witness: solved.witness,
                    guarantee: AlgorithmGuarantee::Approximate,
                });
            }
            SolveDisposition::Infinite { free_vars } => {
                return Ok(MachineSolveResult {
                    disposition: SolveDisposition::Infinite { free_vars },
                    solution: None,
                    witness: solved.witness,
                    guarantee: AlgorithmGuarantee::Approximate,
                });
            }
            SolveDisposition::ResourceLimited => {
                return Ok(MachineSolveResult {
                    disposition: SolveDisposition::ResourceLimited,
                    solution: None,
                    witness: solved.witness,
                    guarantee: AlgorithmGuarantee::Approximate,
                });
            }
        }
    }
    let mut y_data = Vec::with_capacity((n * m) as usize);
    for i in 0..n {
        for j in 0..m {
            y_data.push(y_cols[j as usize][i as usize]);
        }
    }
    let y = MatrixValue::from_f64_row_major(n, m, y_data)?;
    let x = transpose(&y);
    // Residual of X B − A in ∞-norm.
    let xb = super::ops::matmul(&x, b)?;
    let mut residual = 0.0_f64;
    for i in 0..a.shape().rows {
        for j in 0..a.shape().cols {
            let xvi = match xb.get(i, j)? {
                super::value::MatrixEntry::MachineF64(v) => v,
                _ => unreachable!(),
            };
            let avi = match a.get(i, j)? {
                super::value::MatrixEntry::MachineF64(v) => v,
                _ => unreachable!(),
            };
            residual = residual.max((xvi - avi).abs());
        }
    }
    let numerical_rank = last_witness.map(|w| w.numerical_rank).unwrap_or(n);
    let solution = MatrixResult::from_owned(x, AlgorithmGuarantee::Approximate).with_machine_witness(residual, None);
    Ok(MachineSolveResult {
        disposition: SolveDisposition::Unique,
        solution: Some(solution),
        witness: Some(MachineSolveWitness {
            residual_inf: Some(residual),
            numerical_rank,
            pivot_threshold,
        }),
        guarantee: AlgorithmGuarantee::Approximate,
    })
}

/// 机器条件数估计结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachineCondEstimate {
    /// `κ` 估计；奇异 / 秩亏时为 `+∞`。
    pub value: f64,
    /// 数值秩。
    pub numerical_rank: u64,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

fn matrix_as_machine_f64(matrix: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    if matrix.parent().element.is_machine() {
        return Ok(matrix.owning_copy());
    }
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let mut data = Vec::with_capacity((rows * cols) as usize);
    for i in 0..rows {
        for j in 0..cols {
            let v = match matrix.get(i, j)? {
                super::value::MatrixEntry::Integer(z) => z.to_f64_approximate().ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "cond_integer_to_f64")
                })?,
                super::value::MatrixEntry::Rational(r) => r.to_f64_approximate().ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "cond_rational_to_f64")
                })?,
                super::value::MatrixEntry::MachineF64(x) => x,
                super::value::MatrixEntry::ComplexExact { .. } => {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "cond_complex_pending"));
                }
            };
            data.push(v);
        }
    }
    MatrixValue::from_f64_row_major(rows, cols, data)
}

/// 机器条件数估计（方阵；精确输入先提升到 `f64`）。
pub fn condition_number_machine(matrix: &MatrixValue, pivot_threshold: f64) -> Result<MachineCondEstimate, Diagnostic> {
    if !matrix.shape().is_square() {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "cond_requires_square"));
    }
    let machine = matrix_as_machine_f64(matrix)?;
    let lu = lu_partial_pivot(&machine, pivot_threshold)?;
    let value = conditioning_from_u_diag(&lu).unwrap_or(f64::INFINITY);
    Ok(MachineCondEstimate {
        value,
        numerical_rank: lu.numerical_rank,
        guarantee: AlgorithmGuarantee::Approximate,
    })
}

/// 机器数值秩（经 LU）。
pub fn rank_machine(matrix: &MatrixValue, pivot_threshold: f64) -> Result<(u64, AlgorithmGuarantee), Diagnostic> {
    let lu = lu_partial_pivot(matrix, pivot_threshold)?;
    Ok((lu.numerical_rank, AlgorithmGuarantee::Approximate))
}
