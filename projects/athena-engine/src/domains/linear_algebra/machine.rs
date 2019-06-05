//! 机器路径：部分主元 LU、三角求解、残差。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    status::{AlgorithmGuarantee, MachineSolveWitness, SolveDisposition},
    value::MatrixValue,
};

/// 机器 LU 分解（`PA = LU`，`A` 被原地覆盖为组合矩阵）。
#[derive(Debug, Clone, PartialEq)]
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

/// 机器求解结果。
#[derive(Debug, Clone, PartialEq)]
pub struct MachineSolveResult {
    /// 分类。
    pub disposition: SolveDisposition,
    /// 解（`n×1`）。
    pub solution: Option<MatrixValue>,
    /// 残差见证。
    pub witness: Option<MachineSolveWitness>,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
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
                residual_inf: f64::NAN,
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
                    residual_inf: f64::NAN,
                    numerical_rank: lu.numerical_rank,
                    pivot_threshold: lu.pivot_threshold,
                }),
                guarantee: AlgorithmGuarantee::Approximate,
            });
        }
        x[i as usize] /= diag;
    }
    let solution = MatrixValue::from_f64_row_major(n, 1, x)?;
    Ok(MachineSolveResult {
        disposition: SolveDisposition::Unique,
        solution: Some(solution),
        witness: None,
        guarantee: AlgorithmGuarantee::Approximate,
    })
}

/// 机器路径求解并附残差。
pub fn solve_machine(a: &MatrixValue, b: &MatrixValue, pivot_threshold: f64) -> Result<MachineSolveResult, Diagnostic> {
    let lu = lu_partial_pivot(a, pivot_threshold)?;
    let mut result = solve_lu(&lu, b)?;
    if let Some(sol) = &result.solution {
        let ax = super::ops::matmul(a, sol)?;
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
        result.witness = Some(MachineSolveWitness { residual_inf: residual, numerical_rank: lu.numerical_rank, pivot_threshold });
    }
    Ok(result)
}

/// 机器数值秩（经 LU）。
pub fn rank_machine(matrix: &MatrixValue, pivot_threshold: f64) -> Result<(u64, AlgorithmGuarantee), Diagnostic> {
    let lu = lu_partial_pivot(matrix, pivot_threshold)?;
    Ok((lu.numerical_rank, AlgorithmGuarantee::Approximate))
}
