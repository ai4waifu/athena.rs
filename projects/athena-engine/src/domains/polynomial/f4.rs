//! F4 Macaulay 矩阵（Living `04` / `30`）：系数面挂 `athena-ndarray` `Array2d`。
//!
//! 列单项式字典仍属多项式域元数据。系数缓冲经 [`NumberInMemoryStorage`]（GC `try_clone_in`）。
//! 本切片：建矩阵 / 行还原 / 消元 / sugar 选择与继承 / 闭包 / 准则 1+2 / 增量更新 / 证书闭环。
//! Resume 时可携带 sugar 向量（`GroebnerFrontier::candidate_sugars`）。缺省则从 [`polynomial_sugar`] 重算。

use std::collections::{HashMap, HashSet};

use athena_ndarray::{Array2d, MemoryBudget, array2d_from_storage};
use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    builder::PolynomialBuilder,
    coefficient_kernel::CoefficientRing,
    exponent::add_exponent_vectors,
    monomial_layout::MonomialLayout,
    object::{CanonicalPolynomial, Polynomial},
    ring_table::RingTable,
};
use crate::runtime::values::ndarray_number::{NumberInMemoryStorage, dup_number};

/// F4 critical pair（下标相对当前基 · 含 sugar）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct F4CriticalPair {
    /// 基下标 `i`。
    pub i: usize,
    /// 基下标 `j`（约定 `i < j`）。
    pub j: usize,
    /// pair sugar（Giovini 风格初值）。
    pub sugar: u32,
}

/// 符号预处理产出的一行：乘子 × 基多项式下标。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct F4SymbolicRow {
    /// 单项式乘子指数。
    pub multiplier: Vec<u32>,
    /// 基多项式下标。
    pub poly_index: usize,
}

/// 一行 Macaulay 输入：单项式乘子 × 基多项式。
#[derive(Debug, Clone, Copy)]
pub struct MacaulayRowInput<'a> {
    /// 乘子指数（与环变量表等长；全零表示不乘）。
    pub multiplier: &'a [u32],
    /// 基多项式（须与矩阵同环）。
    pub polynomial: &'a Polynomial,
}

/// Macaulay 矩阵：列字典 + 行主序稠密 [`Array2d`] 系数。
#[derive(Debug)]
pub struct MacaulayMatrix {
    /// 所属环。
    pub ring: RingId,
    /// 列单项式指数（降序）。
    pub columns: Vec<Vec<u32>>,
    /// 系数 `shape = [nrows, ncols]`。
    pub coeffs: Array2d<Number, NumberInMemoryStorage>,
}

impl MacaulayMatrix {
    /// 行数。
    pub fn nrows(&self) -> usize {
        self.coeffs.shape().dimensions().first().copied().unwrap_or(0) as usize
    }

    /// 列数。
    pub fn ncols(&self) -> usize {
        self.columns.len()
    }

    fn budget_for(elements: usize) -> Result<MemoryBudget> {
        // `Number` 栈宽作驻留下界；真实 GC 负载另计。测试 / 小矩阵足够。
        let bytes = elements.saturating_mul(std::mem::size_of::<Number>()).saturating_mul(4).max(4096);
        MemoryBudget::new(bytes)
            .map_err(|_| Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_budget"))
    }

    fn flat_index(&self, row: usize, col: usize) -> Result<usize> {
        let off = self.coeffs.row_major_offset(row as u64, col as u64).map_err(|_| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_index_out_of_range")
        })?;
        usize::try_from(off).map_err(|_| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_index_overflow")
        })
    }

    fn get(&self, row: usize, col: usize) -> Result<&Number> {
        let i = self.flat_index(row, col)?;
        self.coeffs
            .store()
            .as_slice()
            .get(i)
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_get_oob"))
    }

    fn set(&mut self, row: usize, col: usize, value: Number) -> Result<()> {
        let i = self.flat_index(row, col)?;
        let slot = self.coeffs.store_mut().as_slice_mut().get_mut(i).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_set_oob")
        })?;
        *slot = value;
        Ok(())
    }
}

/// 从乘子×多项式行构建稠密 Macaulay `Array2d`（无消元）。
pub fn build_macaulay_matrix(rows: &[MacaulayRowInput<'_>], rings: &RingTable) -> Result<MacaulayMatrix> {
    if rows.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_empty_rows"));
    }
    let ring = rows[0].polynomial.ring();
    let desc = rings.get(ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_unknown_ring")
            .detail("ring_id", ring.0.to_string())
    })?;
    let nvars = desc.variables.len();
    let layout = &desc.monomial_layout;
    let coeff = rings.coefficient_kernel(ring)?;

    for row in rows {
        if row.polynomial.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "macaulay_row_ring_mismatch"));
        }
        if row.multiplier.len() != nvars {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "macaulay_multiplier_width_mismatch"));
        }
    }

    let mut column_set: HashSet<Vec<u32>> = HashSet::new();
    let mut expanded: Vec<Vec<(Vec<u32>, Number)>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut terms = Vec::with_capacity(row.polynomial.terms().len());
        for term in row.polynomial.terms() {
            let exponents = add_exponent_vectors(row.multiplier, term.exponents())?;
            column_set.insert(exponents.clone());
            terms.push((exponents, dup_number(term.coefficient())));
        }
        expanded.push(terms);
    }

    let mut columns: Vec<Vec<u32>> = column_set.into_iter().collect();
    columns.sort_by(|a, b| layout.cmp_exponents_desc(a, b));
    let col_of: HashMap<Vec<u32>, usize> = columns.iter().enumerate().map(|(i, e)| (e.clone(), i)).collect();

    let nrows = rows.len() as u64;
    let ncols = columns.len() as u64;
    let nelem = (nrows as usize).saturating_mul(ncols as usize);
    let budget = MacaulayMatrix::budget_for(nelem)?;
    let store = NumberInMemoryStorage::zeros(nelem);
    let mut coeffs = array2d_from_storage(nrows, ncols, store, budget).map_err(|_| {
        Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_array2d_bind")
    })?;

    for (r, terms) in expanded.into_iter().enumerate() {
        let mut accum: HashMap<usize, Number> = HashMap::new();
        for (exponents, next) in terms {
            let c = *col_of.get(&exponents).expect("column registered");
            match accum.remove(&c) {
                Some(prev) => {
                    accum.insert(c, coeff.add(prev, next)?);
                }
                None => {
                    accum.insert(c, next);
                }
            }
        }
        for (c, v) in accum {
            if v.is_zero() {
                continue;
            }
            let i =
                (r as u64).checked_mul(ncols).and_then(|o| o.checked_add(c as u64)).and_then(|o| usize::try_from(o).ok()).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_flat_overflow")
                })?;
            coeffs.store_mut().as_slice_mut()[i] = v;
        }
    }

    Ok(MacaulayMatrix { ring, columns, coeffs })
}

/// 历史名；等同 [`build_macaulay_matrix`]。
pub fn build_macaulay_csr(rows: &[MacaulayRowInput<'_>], rings: &RingTable) -> Result<MacaulayMatrix> {
    build_macaulay_matrix(rows, rings)
}

/// 将第 `row` 行还原为规范多项式。
pub fn macaulay_row_to_polynomial(matrix: &MacaulayMatrix, row: usize, rings: &RingTable) -> Result<CanonicalPolynomial> {
    if row >= matrix.nrows() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_row_out_of_range")
            .detail("row", row.to_string())
            .detail("nrows", matrix.nrows().to_string()));
    }
    let mut builder = PolynomialBuilder::new(matrix.ring);
    for col in 0..matrix.ncols() {
        let v = matrix.get(row, col)?;
        if v.is_zero() {
            continue;
        }
        builder.push_term(dup_number(v), matrix.columns[col].clone())?;
    }
    builder.build(rings)
}

/// 用含列 `pivot_col` 的首个非零行作主元，消去其余行该列（域系数）。
pub fn eliminate_macaulay_column(matrix: &MacaulayMatrix, pivot_col: u32, rings: &RingTable) -> Result<MacaulayMatrix> {
    let pivot_col = pivot_col as usize;
    if pivot_col >= matrix.ncols() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_pivot_col_out_of_range"));
    }
    let coeff = require_field_kernel(matrix.ring, rings)?;
    let mut rows = copy_dense_rows(matrix)?;
    let pivot_row = rows.iter().position(|row| !row[pivot_col].is_zero()).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_pivot_missing")
    })?;
    eliminate_column_in_rows(&mut rows, pivot_row, pivot_col, &coeff)?;
    rebuild_matrix(matrix.ring, &matrix.columns, rows)
}

/// 对整张 Macaulay 矩阵做左到右批量高斯消元（域系数 · Living `30` F4 消元步进）。
///
/// 每列在尚未用作主元的行中选第一个非零元，消去其余行该列，最后丢掉全零行。
/// 不声称完整 F4（无 pair 选择 / sugar / 符号预处理调度）。
pub fn reduce_macaulay_matrix(matrix: &MacaulayMatrix, rings: &RingTable) -> Result<MacaulayMatrix> {
    let coeff = require_field_kernel(matrix.ring, rings)?;
    let mut rows = copy_dense_rows(matrix)?;
    let ncols = matrix.ncols();
    let mut pivot_used = 0usize;
    for col in 0..ncols {
        let Some(rel) = rows[pivot_used..].iter().position(|row| !row[col].is_zero())
        else {
            continue;
        };
        let pivot_row = pivot_used + rel;
        if pivot_row != pivot_used {
            rows.swap(pivot_used, pivot_row);
        }
        eliminate_column_in_rows(&mut rows, pivot_used, col, &coeff)?;
        pivot_used += 1;
        if pivot_used >= rows.len() {
            break;
        }
    }
    rebuild_matrix(matrix.ring, &matrix.columns, rows)
}

/// 将非零行物化为规范多项式候选（顺序与当前矩阵行一致）。
pub fn macaulay_matrix_polynomials(matrix: &MacaulayMatrix, rings: &RingTable) -> Result<Vec<CanonicalPolynomial>> {
    let mut out = Vec::with_capacity(matrix.nrows());
    for r in 0..matrix.nrows() {
        let p = macaulay_row_to_polynomial(matrix, r, rings)?;
        if !p.is_zero() {
            out.push(p);
        }
    }
    Ok(out)
}

fn require_field_kernel<'a>(ring: RingId, rings: &'a RingTable) -> Result<CoefficientRing<'a>> {
    let coeff = rings.coefficient_kernel(ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_eliminate_requires_field"));
    }
    Ok(coeff)
}

fn copy_dense_rows(matrix: &MacaulayMatrix) -> Result<Vec<Vec<Number>>> {
    let nrows = matrix.nrows();
    let ncols = matrix.ncols();
    let mut rows = Vec::with_capacity(nrows);
    for r in 0..nrows {
        let mut row = Vec::with_capacity(ncols);
        for c in 0..ncols {
            row.push(dup_number(matrix.get(r, c)?));
        }
        rows.push(row);
    }
    Ok(rows)
}

fn eliminate_column_in_rows(rows: &mut [Vec<Number>], pivot_row: usize, pivot_col: usize, coeff: &CoefficientRing<'_>) -> Result<()> {
    let ncols = rows[pivot_row].len();
    let pivot_val = dup_number(&rows[pivot_row][pivot_col]);
    for r in 0..rows.len() {
        if r == pivot_row {
            continue;
        }
        if rows[r][pivot_col].is_zero() {
            continue;
        }
        let entry = dup_number(&rows[r][pivot_col]);
        let factor = coeff.div(entry, dup_number(&pivot_val))?;
        for c in 0..ncols {
            let scaled = coeff.mul(dup_number(&factor), dup_number(&rows[pivot_row][c]))?;
            rows[r][c] = coeff.sub(dup_number(&rows[r][c]), scaled)?;
        }
    }
    Ok(())
}

fn rebuild_matrix(ring: RingId, columns: &[Vec<u32>], rows: Vec<Vec<Number>>) -> Result<MacaulayMatrix> {
    let ncols = columns.len();
    let kept: Vec<Vec<Number>> = rows.into_iter().filter(|row| row.iter().any(|v| !v.is_zero())).collect();
    if kept.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_eliminate_all_rows_zero"));
    }
    let new_nrows = kept.len() as u64;
    let nelem = kept.len().saturating_mul(ncols);
    let budget = MacaulayMatrix::budget_for(nelem)?;
    let mut flat = Vec::with_capacity(nelem);
    for row in kept {
        flat.extend(row);
    }
    let store = NumberInMemoryStorage::from_vec(flat);
    let coeffs = array2d_from_storage(new_nrows, ncols as u64, store, budget).map_err(|_| {
        Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "macaulay_array2d_rebind")
    })?;
    Ok(MacaulayMatrix { ring, columns: columns.to_vec(), coeffs })
}

/// 历史类型别名。
pub type MacaulayCsrMatrix = MacaulayMatrix;

/// 多项式初值 sugar：各项总次数最大值（零多项式为 0）。
pub fn polynomial_sugar(poly: &Polynomial) -> u32 {
    poly.terms().iter().map(|t| exponents_total_degree(t.exponents())).max().unwrap_or(0)
}

/// Critical pair sugar：`max(sugar(f)-deg(LM(f)), sugar(g)-deg(LM(g))) + deg(lcm(LM(f),LM(g)))`。
///
/// 初值 sugar 取 [`polynomial_sugar`]。继承 sugar 见 [`pair_sugar_with`]。
pub fn pair_sugar_degree(f: &Polynomial, g: &Polynomial, layout: &MonomialLayout) -> Result<u32> {
    pair_sugar_with(f, g, polynomial_sugar(f), polynomial_sugar(g), layout)
}

/// Critical pair sugar（显式基元素 sugar · Giovini 继承）。
pub fn pair_sugar_with(f: &Polynomial, g: &Polynomial, sugar_f: u32, sugar_g: u32, layout: &MonomialLayout) -> Result<u32> {
    let lf = f.terms().first().ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_pair_sugar_zero_poly")
    })?;
    let lg = g.terms().first().ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_pair_sugar_zero_poly")
    })?;
    let lcm = layout.lcm_exponents(lf.exponents(), lg.exponents())?;
    let deg_lcm = exponents_total_degree(&lcm);
    let deg_lf = exponents_total_degree(lf.exponents());
    let deg_lg = exponents_total_degree(lg.exponents());
    let left = sugar_f.saturating_sub(deg_lf).saturating_add(deg_lcm);
    let right = sugar_g.saturating_sub(deg_lg).saturating_add(deg_lcm);
    Ok(left.max(right))
}

/// 选出所有 sugar 最小的 pending pairs（同 sugar 一并进入 F4 批量）。
///
/// `sugars` 与 `basis` 等长。长度不匹配时退回 [`polynomial_sugar`]。
pub fn select_minimal_sugar_pairs(basis: &[Polynomial], pairs: &[(usize, usize)], layout: &MonomialLayout) -> Result<Vec<F4CriticalPair>> {
    let owned: Vec<u32> = basis.iter().map(polynomial_sugar).collect();
    select_minimal_sugar_pairs_with(basis, &owned, pairs, layout)
}

/// 选出最小 sugar pairs（使用显式 sugar 表）。
pub fn select_minimal_sugar_pairs_with(
    basis: &[Polynomial],
    sugars: &[u32],
    pairs: &[(usize, usize)],
    layout: &MonomialLayout,
) -> Result<Vec<F4CriticalPair>> {
    if pairs.is_empty() {
        return Ok(Vec::new());
    }
    let mut scored = Vec::with_capacity(pairs.len());
    for &(i, j) in pairs {
        let (i, j) = if i < j { (i, j) } else { (j, i) };
        if i == j || j >= basis.len() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "polynomial")
                .detail("operation", "f4_pair_index_out_of_range"));
        }
        let sugar_i = sugars.get(i).copied().unwrap_or_else(|| polynomial_sugar(&basis[i]));
        let sugar_j = sugars.get(j).copied().unwrap_or_else(|| polynomial_sugar(&basis[j]));
        let sugar = pair_sugar_with(&basis[i], &basis[j], sugar_i, sugar_j, layout)?;
        scored.push(F4CriticalPair { i, j, sugar });
    }
    let min_sugar = scored.iter().map(|p| p.sugar).min().expect("non-empty pairs");
    Ok(scored.into_iter().filter(|p| p.sugar == min_sugar).collect())
}

/// 对选定 pairs 做最小符号预处理：每个 pair 贡献两行 `u_i·f_i` 与 `u_j·f_j`。
///
/// `u_* = lcm(LM(i),LM(j)) / LM(*)`。全量 reducer 闭包见 [`symbolic_preprocess_closure`]。
pub fn symbolic_preprocess_pairs(basis: &[Polynomial], pairs: &[F4CriticalPair], layout: &MonomialLayout) -> Result<Vec<F4SymbolicRow>> {
    let mut rows = Vec::with_capacity(pairs.len().saturating_mul(2));
    let mut seen: HashSet<(usize, Vec<u32>)> = HashSet::new();
    for pair in pairs {
        let fi = basis.get(pair.i).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_preprocess_index")
        })?;
        let fj = basis.get(pair.j).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_preprocess_index")
        })?;
        let lf = fi.terms().first().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_preprocess_zero_poly")
        })?;
        let lg = fj.terms().first().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_preprocess_zero_poly")
        })?;
        let lcm = layout.lcm_exponents(lf.exponents(), lg.exponents())?;
        let ui = layout.exponents_delta(&lcm, lf.exponents())?;
        let uj = layout.exponents_delta(&lcm, lg.exponents())?;
        push_unique_symbolic_row(&mut rows, &mut seen, pair.i, ui);
        push_unique_symbolic_row(&mut rows, &mut seen, pair.j, uj);
    }
    if rows.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_preprocess_empty"));
    }
    Ok(rows)
}

/// 全量符号预处理：pair 行闭包后再对出现的可约单项式扩 reducer 行，直至不动点。
pub fn symbolic_preprocess_closure(basis: &[Polynomial], pairs: &[F4CriticalPair], layout: &MonomialLayout) -> Result<Vec<F4SymbolicRow>> {
    let mut rows = symbolic_preprocess_pairs(basis, pairs, layout)?;
    let mut seen: HashSet<(usize, Vec<u32>)> = rows.iter().map(|r| (r.poly_index, r.multiplier.clone())).collect();
    loop {
        let mut monomials: HashSet<Vec<u32>> = HashSet::new();
        for row in &rows {
            let poly = &basis[row.poly_index];
            for term in poly.terms() {
                monomials.insert(layout.add_exponents(row.multiplier.as_slice(), term.exponents())?);
            }
        }
        let mut grew = false;
        for m in monomials {
            for (k, g) in basis.iter().enumerate() {
                let Some(lm) = g.terms().first()
                else {
                    continue;
                };
                if !layout.monomial_divides(lm.exponents(), &m) {
                    continue;
                }
                let mult = layout.exponents_delta(&m, lm.exponents())?;
                let before = seen.len();
                push_unique_symbolic_row(&mut rows, &mut seen, k, mult);
                if seen.len() > before {
                    grew = true;
                }
                break;
            }
        }
        if !grew {
            break;
        }
    }
    Ok(rows)
}

/// 一次 F4 矩阵步进：最小 sugar pairs → 符号预处理 → 建矩阵 → 批量消元 → 提取非零行。
///
/// 不更新基、不维护 frontier、不声称 Verified Gröbner。
pub fn f4_matrix_reduce_pairs(basis: &[Polynomial], pairs: &[(usize, usize)], rings: &RingTable) -> Result<Vec<CanonicalPolynomial>> {
    if basis.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_empty_basis"));
    }
    let ring = basis[0].ring();
    for p in basis {
        if p.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "f4_basis_ring_mismatch"));
        }
    }
    let desc = rings.get(ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "f4_unknown_ring")
            .detail("ring_id", ring.0.to_string())
    })?;
    let layout = &desc.monomial_layout;
    let selected = select_minimal_sugar_pairs(basis, pairs, layout)?;
    if selected.is_empty() {
        return Ok(Vec::new());
    }
    let prep = symbolic_preprocess_closure(basis, &selected, layout)?;
    let row_inputs: Vec<MacaulayRowInput<'_>> =
        prep.iter().map(|row| MacaulayRowInput { multiplier: row.multiplier.as_slice(), polynomial: &basis[row.poly_index] }).collect();
    let matrix = build_macaulay_matrix(&row_inputs, rings)?;
    let reduced = reduce_macaulay_matrix(&matrix, rings)?;
    macaulay_matrix_polynomials(&reduced, rings)
}

/// F4 基更新资源合同。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct F4UpdateLimits {
    /// 最大矩阵步进次数（每步处理一批最小 sugar pairs）。
    pub max_matrix_steps: u32,
    /// 最大基大小。
    pub max_basis_size: u32,
}

impl Default for F4UpdateLimits {
    fn default() -> Self {
        Self { max_matrix_steps: 10_000, max_basis_size: 128 }
    }
}

/// F4 增量更新结果（不声称 Verified Gröbner）。
#[derive(Debug, PartialEq)]
pub enum F4UpdateComputation {
    /// pairs 耗尽。
    Complete {
        /// 当前基。
        basis: Vec<Polynomial>,
        /// 已执行矩阵步数。
        matrix_steps: u32,
    },
    /// 矩阵步预算耗尽。
    Partial {
        /// 当前基。
        basis: Vec<Polynomial>,
        /// 尚未处理 pairs。
        pending_pairs: Vec<(usize, usize)>,
        /// 已执行矩阵步数。
        matrix_steps: u32,
        /// 与 `basis` 等长的 Giovini sugar 表。
        sugars: Vec<u32>,
    },
    /// 基大小触顶。
    ResourceLimited {
        /// 当前基。
        basis: Vec<Polynomial>,
        /// 尚未处理 pairs。
        pending_pairs: Vec<(usize, usize)>,
        /// 待插入但因基上限未入基的多项式。
        pending_insertion: Option<Polynomial>,
        /// 待插入多项式的 sugar。
        pending_insertion_sugar: Option<u32>,
        /// 已执行矩阵步数。
        matrix_steps: u32,
        /// 与 `basis` 等长的 Giovini sugar 表。
        sugars: Vec<u32>,
    },
}

/// 从生成元跑 F4 风格增量基更新：反复最小 sugar 批量矩阵消元，按新 LM 扩基。
///
/// 插入准则：消元行 LM 不被当前基任一 LM 整除则入基并挂新 pairs。
/// 不做自约化、不跑独立 verifier、不写 `GroebnerCertificate`。
pub fn run_f4_basis_update(generators: Vec<Polynomial>, rings: &RingTable, limits: F4UpdateLimits) -> Result<F4UpdateComputation> {
    if generators.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "f4_update_empty_generators"));
    }
    let ring = generators[0].ring();
    for g in &generators {
        if g.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "f4_update_ring_mismatch"));
        }
    }
    require_f4_field(ring, rings)?;
    let mut basis: Vec<Polynomial> = generators.into_iter().filter(|p| !p.is_zero()).collect();
    if basis.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_update_zero_ideal"));
    }
    let mut pending: HashSet<(usize, usize)> = HashSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for i in 0..basis.len() {
        for j in (i + 1)..basis.len() {
            enqueue_f4_pair(i, j, &mut pairs, &mut pending);
        }
    }
    continue_f4_basis_update(basis, pairs, pending, None, 0, None, None, rings, limits)
}

/// 从 Partial / ResourceLimited frontier 恢复 F4 基更新。
///
/// `sugars` 与 `basis` 等长时沿用继承表。否则按 [`polynomial_sugar`] 重算。
pub fn resume_f4_basis_update(
    basis: Vec<Polynomial>,
    pairs_in: Vec<(usize, usize)>,
    pending_insertion: Option<Polynomial>,
    prior_matrix_steps: u32,
    sugars: Option<Vec<u32>>,
    pending_insertion_sugar: Option<u32>,
    rings: &RingTable,
    limits: F4UpdateLimits,
) -> Result<F4UpdateComputation> {
    if basis.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "polynomial").detail("operation", "f4_resume_empty_basis"));
    }
    let ring = basis[0].ring();
    for p in &basis {
        if p.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "f4_resume_ring_mismatch"));
        }
    }
    if let Some(p) = &pending_insertion {
        if p.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "f4_resume_insertion_ring_mismatch"));
        }
    }
    require_f4_field(ring, rings)?;
    let n = basis.len();
    for &(i, j) in &pairs_in {
        if i >= n || j >= n || i == j {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "polynomial")
                .detail("operation", "f4_resume_invalid_pair")
                .detail("i", i.to_string())
                .detail("j", j.to_string())
                .detail("basis_len", n.to_string()));
        }
    }
    if pending_insertion.is_none() && pairs_in.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "f4_resume_no_pending_work"));
    }
    let mut pending: HashSet<(usize, usize)> = HashSet::new();
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (i, j) in pairs_in {
        enqueue_f4_pair(i, j, &mut pairs, &mut pending);
    }
    continue_f4_basis_update(basis, pairs, pending, pending_insertion, prior_matrix_steps, sugars, pending_insertion_sugar, rings, limits)
}

fn require_f4_field(ring: RingId, rings: &RingTable) -> Result<()> {
    let coeff = rings.coefficient_kernel(ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "f4_update_requires_field"));
    }
    Ok(())
}

fn continue_f4_basis_update(
    mut basis: Vec<Polynomial>,
    mut pairs: Vec<(usize, usize)>,
    mut pending: HashSet<(usize, usize)>,
    mut pending_insertion: Option<Polynomial>,
    mut matrix_steps: u32,
    initial_sugars: Option<Vec<u32>>,
    mut pending_insertion_sugar: Option<u32>,
    rings: &RingTable,
    limits: F4UpdateLimits,
) -> Result<F4UpdateComputation> {
    let ring = basis[0].ring();
    let desc = rings.get(ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "f4_update_unknown_ring")
            .detail("ring_id", ring.0.to_string())
    })?;
    let layout = &desc.monomial_layout;
    let mut sugars: Vec<u32> = match initial_sugars {
        Some(s) if s.len() == basis.len() => s,
        _ => basis.iter().map(polynomial_sugar).collect(),
    };

    if let Some(remainder) = pending_insertion.take() {
        if basis.len() as u32 >= limits.max_basis_size {
            return Ok(F4UpdateComputation::ResourceLimited {
                basis,
                pending_pairs: pairs_from_f4_pending(&pending),
                pending_insertion: Some(remainder),
                pending_insertion_sugar,
                matrix_steps,
                sugars,
            });
        }
        let sugar = pending_insertion_sugar.take().unwrap_or_else(|| polynomial_sugar(&remainder));
        let idx = basis.len();
        basis.push(remainder);
        sugars.push(sugar);
        for k in 0..idx {
            enqueue_f4_pair(k, idx, &mut pairs, &mut pending);
        }
    }

    while !pairs.is_empty() {
        // Buchberger criterion 1: coprime leading monomials ⇒ S-pair → 0（不占矩阵步）。
        // Buchberger criterion 2 (chain): 侧边 pairs 已处理且第三 LM 整除 lcm。
        pairs.retain(|&(i, j)| {
            let key = ordered_f4_pair(i, j);
            if leading_monomials_coprime(&basis[i], &basis[j]) {
                pending.remove(&key);
                return false;
            }
            match chain_criterion_applies(&basis, i, j, &pending, layout) {
                Ok(true) => {
                    pending.remove(&key);
                    false
                }
                Ok(false) => true,
                Err(_) => true,
            }
        });
        if pairs.is_empty() {
            break;
        }
        if matrix_steps >= limits.max_matrix_steps {
            return Ok(F4UpdateComputation::Partial { basis, pending_pairs: pairs_from_f4_pending(&pending), matrix_steps, sugars });
        }
        let selected = select_minimal_sugar_pairs_with(&basis, &sugars, &pairs, layout)?;
        if selected.is_empty() {
            break;
        }
        let batch_sugar = selected[0].sugar;
        let selected_keys: HashSet<(usize, usize)> = selected.iter().map(|p| (p.i, p.j)).collect();
        let selected_pairs: Vec<(usize, usize)> = selected.iter().map(|p| (p.i, p.j)).collect();
        let extracted = f4_matrix_reduce_pairs(&basis, &selected_pairs, rings)?;
        matrix_steps = matrix_steps.saturating_add(1);

        pairs.retain(|&(i, j)| {
            let key = ordered_f4_pair(i, j);
            !selected_keys.contains(&key)
        });
        for key in &selected_keys {
            pending.remove(key);
        }

        for cand in extracted {
            if cand.is_zero() {
                continue;
            }
            if lm_divisible_by_basis_lm(&cand, &basis, layout) {
                continue;
            }
            if basis.len() as u32 >= limits.max_basis_size {
                return Ok(F4UpdateComputation::ResourceLimited {
                    basis,
                    pending_pairs: pairs_from_f4_pending(&pending),
                    pending_insertion: Some(cand),
                    pending_insertion_sugar: Some(batch_sugar),
                    matrix_steps,
                    sugars,
                });
            }
            let idx = basis.len();
            basis.push(cand);
            // 新基元素继承本批 pair sugar（Giovini）。
            sugars.push(batch_sugar);
            for k in 0..idx {
                enqueue_f4_pair(k, idx, &mut pairs, &mut pending);
            }
        }
    }

    Ok(F4UpdateComputation::Complete { basis, matrix_steps })
}

fn lm_divisible_by_basis_lm(poly: &Polynomial, basis: &[Polynomial], layout: &MonomialLayout) -> bool {
    let Some(lm) = poly.terms().first()
    else {
        return true;
    };
    basis.iter().any(|g| g.terms().first().is_some_and(|lt| layout.monomial_divides(lt.exponents(), lm.exponents())))
}

/// Buchberger first criterion: `LM(f)` and `LM(g)` coprime ⇒ `S(f,g)` → 0.
fn leading_monomials_coprime(f: &Polynomial, g: &Polynomial) -> bool {
    let Some(lf) = f.terms().first()
    else {
        return false;
    };
    let Some(lg) = g.terms().first()
    else {
        return false;
    };
    if lf.exponents().len() != lg.exponents().len() {
        return false;
    }
    lf.exponents().iter().zip(lg.exponents().iter()).all(|(a, b)| *a == 0 || *b == 0)
}

/// Buchberger chain criterion: ∃`k` s.t. `LM(bk) | lcm(LM(bi), LM(bj))` and pairs `(i,k)`, `(j,k)` already treated.
fn chain_criterion_applies(
    basis: &[Polynomial],
    i: usize,
    j: usize,
    pending: &HashSet<(usize, usize)>,
    layout: &MonomialLayout,
) -> Result<bool> {
    let Some(li) = basis[i].terms().first()
    else {
        return Ok(false);
    };
    let Some(lj) = basis[j].terms().first()
    else {
        return Ok(false);
    };
    let lcm_ij = layout.lcm_exponents(li.exponents(), lj.exponents())?;
    let lcm_packed = layout.pack(&lcm_ij)?;
    for k in 0..basis.len() {
        if k == i || k == j {
            continue;
        }
        let Some(lk) = basis[k].terms().first()
        else {
            continue;
        };
        let lk_packed = layout.pack(lk.exponents())?;
        if !layout.packed_divides(&lk_packed, &lcm_packed)? {
            continue;
        }
        if !pending.contains(&ordered_f4_pair(i, k)) && !pending.contains(&ordered_f4_pair(j, k)) {
            return Ok(true);
        }
    }
    Ok(false)
}

fn ordered_f4_pair(i: usize, j: usize) -> (usize, usize) {
    if i < j { (i, j) } else { (j, i) }
}

fn enqueue_f4_pair(i: usize, j: usize, pairs: &mut Vec<(usize, usize)>, pending: &mut HashSet<(usize, usize)>) {
    if i == j {
        return;
    }
    let key = ordered_f4_pair(i, j);
    if pending.insert(key) {
        pairs.push(key);
    }
}

fn pairs_from_f4_pending(pending: &HashSet<(usize, usize)>) -> Vec<(usize, usize)> {
    let mut out: Vec<(usize, usize)> = pending.iter().copied().collect();
    out.sort_unstable();
    out
}

fn push_unique_symbolic_row(rows: &mut Vec<F4SymbolicRow>, seen: &mut HashSet<(usize, Vec<u32>)>, poly_index: usize, multiplier: Vec<u32>) {
    let key = (poly_index, multiplier.clone());
    if seen.insert(key) {
        rows.push(F4SymbolicRow { multiplier, poly_index });
    }
}

fn exponents_total_degree(exponents: &[u32]) -> u32 {
    exponents.iter().fold(0u32, |acc, &e| acc.saturating_add(e))
}
