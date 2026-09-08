//! 精确路径：`ℚ` Gaussian 消元 / RREF / 秩 / 求解，以及 `ℤ` Bareiss。

use athena_numeric::{Integer, Number, Rational, sqrt as num_sqrt};
use athena_types::{Diagnostic, DiagnosticCode};

use super::{
    shape::{MatrixShape, StorageOrder},
    status::{AlgorithmGuarantee, SolveDisposition},
    value::{MatrixEntry, MatrixValue},
};
use crate::runtime::values::numeric_clone::{clone_integer, clone_rational, clone_rationals, resize_rationals};

/// 精确秩结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExactRankResult {
    /// 秩。
    pub rank: u64,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

/// 精确行列式结果。
#[derive(Debug, PartialEq, Eq)]
pub struct ExactDetResult {
    /// 行列式（有理；整数矩阵时分母为 1）。
    pub det: Rational,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

/// 精确矩阵迹结果。
#[derive(Debug, PartialEq, Eq)]
pub struct ExactTraceResult {
    /// 主对角元之和（有理）。
    pub value: Rational,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

/// 精确欧几里得范数结果（仅完美平方根可交付）。
#[derive(Debug, PartialEq, Eq)]
pub struct ExactNormResult {
    /// 范数。
    pub value: Rational,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

/// 精确线性求解结果。
#[derive(Debug, PartialEq)]
pub struct ExactSolveResult {
    /// 分类。
    pub disposition: SolveDisposition,
    /// 特解（`Unique` / `Infinite` 时存在；列为解向量，shape `n×1`）。
    pub particular: Option<MatrixValue>,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

/// RREF 结果。
#[derive(Debug, PartialEq)]
pub struct ExactRrefResult {
    /// 行最简形。
    pub matrix: MatrixValue,
    /// 主元列（0-based）。
    pub pivot_cols: Vec<u64>,
    /// 秩。
    pub rank: u64,
    /// 保证级别。
    pub guarantee: AlgorithmGuarantee,
}

impl ExactDetResult {
    /// Owning 复制（禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self { det: clone_rational(&self.det), guarantee: self.guarantee }
    }
}

impl ExactTraceResult {
    /// Owning 复制（禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self { value: clone_rational(&self.value), guarantee: self.guarantee }
    }
}

impl ExactNormResult {
    /// Owning 复制（禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self { value: clone_rational(&self.value), guarantee: self.guarantee }
    }
}

impl ExactSolveResult {
    /// Owning 复制（禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            disposition: self.disposition.owning_copy(),
            particular: self.particular.as_ref().map(|m| m.owning_copy()),
            guarantee: self.guarantee,
        }
    }
}

impl ExactRrefResult {
    /// Owning 复制（禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self { matrix: self.matrix.owning_copy(), pivot_cols: self.pivot_cols.clone(), rank: self.rank, guarantee: self.guarantee }
    }
}

fn get_q(a: &[Rational], cols: u64, r: u64, c: u64) -> Rational {
    clone_rational(&a[(r * cols + c) as usize])
}

fn set_q(a: &mut [Rational], cols: u64, r: u64, c: u64, v: Rational) {
    a[(r * cols + c) as usize] = v;
}

fn swap_rows(a: &mut [Rational], cols: u64, r1: u64, r2: u64) {
    if r1 == r2 {
        return;
    }
    for c in 0..cols {
        let i = (r1 * cols + c) as usize;
        let j = (r2 * cols + c) as usize;
        a.swap(i, j);
    }
}

/// `ℚ` 上带部分主元的 RREF。
pub fn rref_rational(matrix: &MatrixValue) -> Result<ExactRrefResult, Diagnostic> {
    if matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "rref_requires_exact"));
    }
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let mut a = matrix.to_rationals_row_major()?;
    let mut pivot_cols = Vec::new();
    let mut row = 0u64;
    for col in 0..cols {
        if row >= rows {
            break;
        }
        let mut pivot = row;
        for r in row..rows {
            if !get_q(&a, cols, r, col).is_zero() {
                pivot = r;
                break;
            }
        }
        if get_q(&a, cols, pivot, col).is_zero() {
            continue;
        }
        swap_rows(&mut a, cols, row, pivot);
        let piv = get_q(&a, cols, row, col);
        for c in col..cols {
            let v = get_q(&a, cols, row, c);
            set_q(&mut a, cols, row, c, v.try_div(&piv)?);
        }
        for r in 0..rows {
            if r == row {
                continue;
            }
            let factor = get_q(&a, cols, r, col);
            if factor.is_zero() {
                continue;
            }
            for c in col..cols {
                let v = get_q(&a, cols, r, c).sub(&factor.mul(&get_q(&a, cols, row, c)));
                set_q(&mut a, cols, r, c, v);
            }
        }
        pivot_cols.push(col);
        row += 1;
    }
    let rank = pivot_cols.len() as u64;
    Ok(ExactRrefResult {
        matrix: MatrixValue::from_rationals_row_major(rows, cols, a)?,
        pivot_cols,
        rank,
        guarantee: AlgorithmGuarantee::Exact,
    })
}

/// 精确矩阵秩。
pub fn rank_exact(matrix: &MatrixValue) -> Result<ExactRankResult, Diagnostic> {
    let rref = rref_rational(matrix)?;
    Ok(ExactRankResult { rank: rref.rank, guarantee: AlgorithmGuarantee::Exact })
}

/// Bareiss 分式自由行列式（`ℤ` 或提升后的整数有理矩阵要求分母全为 1）。
pub fn det_bareiss(matrix: &MatrixValue) -> Result<ExactDetResult, Diagnostic> {
    if !matrix.shape().is_square() {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "det_requires_square"));
    }
    if matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "bareiss_requires_exact"));
    }
    let n = matrix.shape().rows;
    if n == 0 {
        return Ok(ExactDetResult { det: Rational::one(), guarantee: AlgorithmGuarantee::Exact });
    }
    let rats = matrix.to_rationals_row_major()?;
    let mut a: Vec<Integer> = Vec::with_capacity(rats.len());
    for r in &rats {
        if !r.is_integer() {
            // 回退到 Q 上 PLU 风格积主元（经 RREF 不可直接得 det 符号积）；改用 Gaussian 消元积。
            return det_rational_product(matrix);
        }
        a.push(r.numerator());
    }
    let mut sign = Integer::one();
    let mut prev = Integer::one();
    for k in 0..n.saturating_sub(1) {
        // ℤ 上部分主元
        let mut piv = k;
        for i in k..n {
            if !a[(i * n + k) as usize].is_zero() {
                piv = i;
                break;
            }
        }
        if a[(piv * n + k) as usize].is_zero() {
            return Ok(ExactDetResult { det: Rational::zero(), guarantee: AlgorithmGuarantee::Exact });
        }
        if piv != k {
            for j in 0..n {
                let i1 = (k * n + j) as usize;
                let i2 = (piv * n + j) as usize;
                a.swap(i1, i2);
            }
            sign = sign.neg();
        }
        for i in (k + 1)..n {
            for j in (k + 1)..n {
                let num = a[(i * n + j) as usize].mul(&a[(k * n + k) as usize]).sub(&a[(i * n + k) as usize].mul(&a[(k * n + j) as usize]));
                // 对 prev 做精确整除
                a[(i * n + j) as usize] = num.div(&prev).expect("div");
            }
        }
        prev = clone_integer(&a[(k * n + k) as usize]);
    }
    let det_z = sign.mul(&a[((n - 1) * n + (n - 1)) as usize]);
    Ok(ExactDetResult { det: Rational::from_integer(det_z), guarantee: AlgorithmGuarantee::Exact })
}

fn det_rational_product(matrix: &MatrixValue) -> Result<ExactDetResult, Diagnostic> {
    let n = matrix.shape().rows;
    let mut a = matrix.to_rationals_row_major()?;
    let mut det = Rational::one();
    for k in 0..n {
        let mut piv = k;
        for i in k..n {
            if !get_q(&a, n, i, k).is_zero() {
                piv = i;
                break;
            }
        }
        if get_q(&a, n, piv, k).is_zero() {
            return Ok(ExactDetResult { det: Rational::zero(), guarantee: AlgorithmGuarantee::Exact });
        }
        if piv != k {
            swap_rows(&mut a, n, k, piv);
            det = det.neg();
        }
        let pivot = get_q(&a, n, k, k);
        det = det.mul(&pivot);
        for i in (k + 1)..n {
            let factor = get_q(&a, n, i, k).try_div(&pivot)?;
            for j in k..n {
                let v = get_q(&a, n, i, j).sub(&factor.mul(&get_q(&a, n, k, j)));
                set_q(&mut a, n, i, j, v);
            }
        }
    }
    Ok(ExactDetResult { det, guarantee: AlgorithmGuarantee::Exact })
}

/// 求解 `A x = b`（`b` 为 `m×1`），精确路径。
pub fn solve_exact(a: &MatrixValue, b: &MatrixValue) -> Result<ExactSolveResult, Diagnostic> {
    if a.parent().element.is_machine() || b.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "solve_exact_rejects_machine"));
    }
    if b.shape().cols != 1 || b.shape().rows != a.shape().rows {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "b_must_be_m_by_1"));
    }
    let m = a.shape().rows;
    let n = a.shape().cols;
    // 增广矩阵 [A|b]
    let mut aug = Vec::with_capacity((m * (n + 1)) as usize);
    let ar = a.to_rationals_row_major()?;
    let br = b.to_rationals_row_major()?;
    for i in 0..m {
        for j in 0..n {
            aug.push(clone_rational(&ar[(i * n + j) as usize]));
        }
        aug.push(clone_rational(&br[i as usize]));
    }
    let cols = n + 1;
    let mut pivot_cols = Vec::new();
    let mut row = 0u64;
    for col in 0..n {
        if row >= m {
            break;
        }
        let mut pivot = row;
        for r in row..m {
            if !get_q(&aug, cols, r, col).is_zero() {
                pivot = r;
                break;
            }
        }
        if get_q(&aug, cols, pivot, col).is_zero() {
            continue;
        }
        swap_rows(&mut aug, cols, row, pivot);
        let piv = get_q(&aug, cols, row, col);
        for c in col..cols {
            let v = get_q(&aug, cols, row, c);
            set_q(&mut aug, cols, row, c, v.try_div(&piv)?);
        }
        for r in 0..m {
            if r == row {
                continue;
            }
            let factor = get_q(&aug, cols, r, col);
            if factor.is_zero() {
                continue;
            }
            for c in col..cols {
                let v = get_q(&aug, cols, r, c).sub(&factor.mul(&get_q(&aug, cols, row, c)));
                set_q(&mut aug, cols, r, c, v);
            }
        }
        pivot_cols.push(col);
        row += 1;
    }
    // 不一致：主元落在增广列
    for r in 0..m {
        let mut all_zero = true;
        for c in 0..n {
            if !get_q(&aug, cols, r, c).is_zero() {
                all_zero = false;
                break;
            }
        }
        if all_zero && !get_q(&aug, cols, r, n).is_zero() {
            return Ok(ExactSolveResult {
                disposition: SolveDisposition::Inconsistent,
                particular: None,
                guarantee: AlgorithmGuarantee::Exact,
            });
        }
    }
    let mut free_vars = Vec::new();
    let mut is_pivot = vec![false; n as usize];
    for &p in &pivot_cols {
        is_pivot[p as usize] = true;
    }
    for j in 0..n {
        if !is_pivot[j as usize] {
            free_vars.push(j);
        }
    }
    let mut x = {
        let mut __v = Vec::new();
        resize_rationals(&mut __v, n as usize, &Rational::zero());
        __v
    };
    for (i, &pc) in pivot_cols.iter().enumerate() {
        x[pc as usize] = get_q(&aug, cols, i as u64, n);
    }
    let particular = MatrixValue::from_rationals_row_major(n, 1, x)?;
    if free_vars.is_empty() {
        Ok(ExactSolveResult { disposition: SolveDisposition::Unique, particular: Some(particular), guarantee: AlgorithmGuarantee::Exact })
    }
    else {
        Ok(ExactSolveResult {
            disposition: SolveDisposition::Infinite { free_vars },
            particular: Some(particular),
            guarantee: AlgorithmGuarantee::Exact,
        })
    }
}

/// 精确求逆：逐列求解 `A X = I`。
pub fn invert_exact(matrix: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    if matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "invert_exact_rejects_machine"));
    }
    if !matrix.shape().is_square() {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch).detail("reason", "invert_requires_square"));
    }
    let n = matrix.shape().rows;
    if n == 0 {
        return MatrixValue::zeros(matrix.parent(), MatrixShape::new(0, 0), StorageOrder::RowMajor);
    }
    let eye = MatrixValue::identity(matrix.parent(), n)?;
    let mut columns: Vec<Vec<Rational>> = Vec::with_capacity(n as usize);
    for j in 0..n {
        let mut b_data = Vec::with_capacity(n as usize);
        for i in 0..n {
            match eye.get(i, j)? {
                MatrixEntry::Integer(z) => b_data.push(Rational::from_integer(z)),
                MatrixEntry::Rational(r) => b_data.push(r),
                MatrixEntry::MachineF64(_) => {
                    return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "invert_eye_machine"));
                }
            }
        }
        let b = MatrixValue::from_rationals_row_major(n, 1, b_data)?;
        let solved = solve_exact(matrix, &b)?;
        match solved.disposition {
            SolveDisposition::Unique => {
                let particular = solved.particular.ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "invert_missing_particular")
                })?;
                columns.push(particular.to_rationals_row_major()?);
            }
            _ => {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("reason", "invert_singular_or_underdetermined")
                    .detail("disposition", format!("{:?}", solved.disposition)));
            }
        }
    }
    let mut data = Vec::with_capacity((n * n) as usize);
    for i in 0..n {
        for j in 0..n {
            data.push(clone_rational(&columns[j as usize][i as usize]));
        }
    }
    MatrixValue::from_rationals_row_major(n, n, data)
}

/// 精确矩阵迹：主对角元之和（取 `min(rows, cols)`）。
pub fn trace_exact(matrix: &MatrixValue) -> Result<ExactTraceResult, Diagnostic> {
    if matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "trace_exact_rejects_machine"));
    }
    let n = matrix.shape().rows.min(matrix.shape().cols);
    let mut sum = Rational::zero();
    for i in 0..n {
        match matrix.get(i, i)? {
            MatrixEntry::Integer(z) => {
                sum = sum.add(&Rational::from_integer(z));
            }
            MatrixEntry::Rational(r) => {
                sum = sum.add(&r);
            }
            MatrixEntry::MachineF64(_) => {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "trace_entry_machine"));
            }
        }
    }
    Ok(ExactTraceResult { value: sum, guarantee: AlgorithmGuarantee::Exact })
}

/// 精确欧几里得 2-范数（行/列向量）；仅完美平方根可交付。
pub fn norm2_exact(matrix: &MatrixValue) -> Result<ExactNormResult, Diagnostic> {
    if matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "norm2_exact_rejects_machine"));
    }
    let comps = vector_rationals_any(matrix)?;
    let mut sum_sq = Rational::zero();
    for c in &comps {
        sum_sq = sum_sq.add(&c.mul(c));
    }
    let Some(root) = (match num_sqrt(&Number::from_rational_normalized(clone_rational(&sum_sq))) {
        Ok(Some(Number::Rational(r))) => Some(r),
        Ok(Some(Number::Integer(z))) => Some(Rational::from_integer(z)),
        _ => None,
    })
    else {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("reason", "norm2_non_perfect_square"));
    };
    Ok(ExactNormResult { value: root, guarantee: AlgorithmGuarantee::Exact })
}

fn vector_rationals_any(matrix: &MatrixValue) -> Result<Vec<Rational>, Diagnostic> {
    let rows = matrix.shape().rows;
    let cols = matrix.shape().cols;
    let (n, as_row) = if rows == 1 && cols >= 1 {
        (cols, true)
    }
    else if cols == 1 && rows >= 1 {
        (rows, false)
    }
    else {
        return Err(Diagnostic::new(DiagnosticCode::ShapeMismatch)
            .detail("reason", "norm_requires_vector")
            .detail("shape", format!("{rows}x{cols}")));
    };
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let entry = if as_row { matrix.get(0, i)? } else { matrix.get(i, 0)? };
        out.push(match entry {
            MatrixEntry::Integer(z) => Rational::from_integer(z),
            MatrixEntry::Rational(r) => r,
            MatrixEntry::MachineF64(_) => {
                return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "norm_entry_machine"));
            }
        });
    }
    Ok(out)
}

/// 精确零空间基：RREF 自由列各生成一个行向量（Mathematica `NullSpace` 形状）。
pub fn nullspace_exact(matrix: &MatrixValue) -> Result<MatrixValue, Diagnostic> {
    if matrix.parent().element.is_machine() {
        return Err(Diagnostic::new(DiagnosticCode::TypeMismatch).detail("reason", "nullspace_exact_rejects_machine"));
    }
    let rref = rref_rational(matrix)?;
    let cols = matrix.shape().cols;
    let mut is_pivot = vec![false; cols as usize];
    for &p in &rref.pivot_cols {
        if (p as usize) < is_pivot.len() {
            is_pivot[p as usize] = true;
        }
    }
    let free_cols: Vec<u64> = (0..cols).filter(|&j| !is_pivot[j as usize]).collect();
    if free_cols.is_empty() {
        return MatrixValue::from_rationals_row_major(0, cols, Vec::new());
    }
    let rref_rats = rref.matrix.to_rationals_row_major()?;
    let rref_cols = rref.matrix.shape().cols;
    let mut data = Vec::with_capacity(free_cols.len() * cols as usize);
    for &free_j in &free_cols {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            row.push(if j == free_j { Rational::one() } else { Rational::zero() });
        }
        for (row_i, &piv_col) in rref.pivot_cols.iter().enumerate() {
            let coeff = clone_rational(&rref_rats[(row_i as u64 * rref_cols + free_j) as usize]);
            row[piv_col as usize] = coeff.neg();
        }
        data.append(&mut row);
    }
    MatrixValue::from_rationals_row_major(free_cols.len() as u64, cols, data)
}
