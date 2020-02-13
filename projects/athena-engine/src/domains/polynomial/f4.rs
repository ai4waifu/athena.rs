//! F4 Macaulay 稀疏矩阵脚手架（Living `04` / `30`）。
//!
//! 本切片只建列字典与 CSR 行载荷，**不做**批量高斯消元 / 完整 F4 主循环。

use std::collections::{HashMap, HashSet};

use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    builder::PolynomialBuilder, exponent::add_exponent_vectors, object::CanonicalPolynomial, object::Polynomial, ring_table::RingTable,
};
use crate::runtime::values::numeric_clone::clone_number;

/// 一行 Macaulay 输入：单项式乘子 × 基多项式。
#[derive(Debug, Clone, Copy)]
pub struct MacaulayRowInput<'a> {
    /// 乘子指数（与环变量表等长；全零表示不乘）。
    pub multiplier: &'a [u32],
    /// 基多项式（须与矩阵同环）。
    pub polynomial: &'a Polynomial,
}

/// 稀疏 Macaulay 矩阵（CSR：`row_ptr` / `col_idx` / `values`）。
///
/// 列按环单项式序**降序**排列（F4 习惯：高次在左）。
#[derive(Debug, PartialEq)]
pub struct MacaulayCsrMatrix {
    /// 所属环。
    pub ring: RingId,
    /// 列单项式指数（与 `col_idx` 对齐）。
    pub columns: Vec<Vec<u32>>,
    /// CSR 行指针，长度 `nrows + 1`。
    pub row_ptr: Vec<usize>,
    /// CSR 列下标。
    pub col_idx: Vec<u32>,
    /// CSR 非零系数（与 `col_idx` 等长）。
    pub values: Vec<Number>,
}

impl MacaulayCsrMatrix {
    /// 行数。
    pub fn nrows(&self) -> usize {
        self.row_ptr.len().saturating_sub(1)
    }

    /// 列数。
    pub fn ncols(&self) -> usize {
        self.columns.len()
    }

    /// 非零元个数。
    pub fn nnz(&self) -> usize {
        self.col_idx.len()
    }
}

/// 从乘子×多项式行构建稀疏 Macaulay CSR（无消元）。
pub fn build_macaulay_csr(rows: &[MacaulayRowInput<'_>], rings: &RingTable) -> Result<MacaulayCsrMatrix> {
    if rows.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_empty_rows"));
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
            terms.push((exponents, clone_number(term.coefficient())));
        }
        expanded.push(terms);
    }

    let mut columns: Vec<Vec<u32>> = column_set.into_iter().collect();
    columns.sort_by(|a, b| layout.cmp_exponents_desc(a, b));
    let col_of: HashMap<Vec<u32>, u32> = columns.iter().enumerate().map(|(i, e)| (e.clone(), i as u32)).collect();

    let mut row_ptr = Vec::with_capacity(rows.len() + 1);
    let mut col_idx = Vec::new();
    let mut values = Vec::new();
    row_ptr.push(0);
    for terms in expanded {
        let mut accum: HashMap<u32, Number> = HashMap::new();
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
        let mut entries: Vec<(u32, Number)> = accum.into_iter().collect();
        entries.sort_by_key(|(c, _)| *c);
        for (c, v) in entries {
            if v.is_zero() {
                continue;
            }
            col_idx.push(c);
            values.push(v);
        }
        row_ptr.push(col_idx.len());
    }

    Ok(MacaulayCsrMatrix { ring, columns, row_ptr, col_idx, values })
}

/// 将 CSR 第 `row` 行还原为规范多项式（列单项式 × 系数）。
pub fn macaulay_row_to_polynomial(matrix: &MacaulayCsrMatrix, row: usize, rings: &RingTable) -> Result<CanonicalPolynomial> {
    if row >= matrix.nrows() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_row_out_of_range")
            .detail("row", row.to_string())
            .detail("nrows", matrix.nrows().to_string()));
    }
    let start = matrix.row_ptr[row];
    let end = matrix.row_ptr[row + 1];
    let mut builder = PolynomialBuilder::new(matrix.ring);
    for k in start..end {
        let col = matrix.col_idx[k] as usize;
        if col >= matrix.columns.len() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "polynomial")
                .detail("operation", "macaulay_col_out_of_range"));
        }
        builder.push_term(clone_number(&matrix.values[k]), matrix.columns[col].clone())?;
    }
    builder.build(rings)
}

/// 用含列 `pivot_col` 的首个非零行作主元，消去其余行该列（域系数；Living `30` F4 消元起步）。
///
/// 不改变列字典；全零行丢弃。完整 F4 符号约化 / 新基插入仍属后续切片。
pub fn eliminate_macaulay_column(matrix: &MacaulayCsrMatrix, pivot_col: u32, rings: &RingTable) -> Result<MacaulayCsrMatrix> {
    if (pivot_col as usize) >= matrix.ncols() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_pivot_col_out_of_range"));
    }
    let coeff = rings.coefficient_kernel(matrix.ring)?;
    if !coeff.is_field() {
        return Err(Diagnostic::new(DiagnosticCode::PolynomialNonFieldDivision)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_eliminate_requires_field"));
    }

    let mut rows: Vec<HashMap<u32, Number>> = Vec::with_capacity(matrix.nrows());
    for r in 0..matrix.nrows() {
        let mut map = HashMap::new();
        let start = matrix.row_ptr[r];
        let end = matrix.row_ptr[r + 1];
        for k in start..end {
            map.insert(matrix.col_idx[k], clone_number(&matrix.values[k]));
        }
        rows.push(map);
    }

    let pivot_row = rows
        .iter()
        .position(|row| row.get(&pivot_col).is_some_and(|v| !v.is_zero()))
        .ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "polynomial")
                .detail("operation", "macaulay_pivot_missing")
        })?;
    let pivot_val = clone_number(rows[pivot_row].get(&pivot_col).expect("pivot present"));

    for r in 0..rows.len() {
        if r == pivot_row {
            continue;
        }
        let Some(entry) = rows[r].get(&pivot_col).filter(|v| !v.is_zero()).map(clone_number) else {
            continue;
        };
        let factor = coeff.div(entry, clone_number(&pivot_val))?;
        let pivot_entries: Vec<(u32, Number)> = rows[pivot_row].iter().map(|(c, v)| (*c, clone_number(v))).collect();
        for (c, pv) in pivot_entries {
            let scaled = coeff.mul(clone_number(&factor), pv)?;
            match rows[r].remove(&c) {
                Some(cur) => {
                    let next = coeff.sub(cur, scaled)?;
                    if !next.is_zero() {
                        rows[r].insert(c, next);
                    }
                }
                None => {
                    let neg = coeff.neg(scaled)?;
                    if !neg.is_zero() {
                        rows[r].insert(c, neg);
                    }
                }
            }
        }
    }

    let mut row_ptr = vec![0usize];
    let mut col_idx = Vec::new();
    let mut values = Vec::new();
    for row in rows {
        if row.is_empty() || row.values().all(|v| v.is_zero()) {
            continue;
        }
        let mut entries: Vec<(u32, Number)> = row.into_iter().filter(|(_, v)| !v.is_zero()).collect();
        entries.sort_by_key(|(c, _)| *c);
        for (c, v) in entries {
            col_idx.push(c);
            values.push(v);
        }
        row_ptr.push(col_idx.len());
    }
    if row_ptr.len() == 1 {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "macaulay_eliminate_all_rows_zero"));
    }

    Ok(MacaulayCsrMatrix {
        ring: matrix.ring,
        columns: matrix.columns.iter().map(|c| c.clone()).collect(),
        row_ptr,
        col_idx,
        values,
    })
}
