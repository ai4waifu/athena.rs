//! 多项式表示族 — 算法可按场景选用，经 canonical [`Polynomial`] 保持数学相等。

use athena_numeric::{Integer, Number};
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    canonical::canonicalize_terms,
    expr::{MonomialTerm, Polynomial},
    ring_table::RingTable,
};
use crate::numeric_clone::{clone_number, clone_numbers, resize_numbers};

/// 目标表示（转换时指定）。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReprTarget {
    /// 单变量稠密系数向量 `coeffs[d] = coeff(x^d)`，无尾随零。
    DenseUnivariate {
        /// 环变量表下标。
        var_index: usize,
    },
    /// 单变量稀疏 `(degree, coeff)`，按 degree 降序、无零系数、无重复 degree。
    SparseUnivariate {
        /// 环变量表下标。
        var_index: usize,
    },
    /// 多变量 distributed sparse（与 canonical [`Polynomial`] 同形）。
    DistributedSparse,
}

/// 算法面向的多项式表示（环 id 与 [`Polynomial`] 一致）。
#[derive(Debug, PartialEq)]
pub struct PolynomialRepr {
    /// 所属环。
    pub ring: RingId,
    /// 具体表示。
    pub body: PolynomialReprBody,
}

/// 表示族变体（不含环 id）。
#[derive(Debug, PartialEq)]
pub enum PolynomialReprBody {
    /// 单变量稠密。
    DenseUnivariate {
        /// 主变量下标。
        var_index: usize,
        /// `coefficients[d]` 为 `x^d` 的系数；零多项式为空向量。
        coefficients: Vec<Number>,
    },
    /// 单变量稀疏。
    SparseUnivariate {
        /// 主变量下标。
        var_index: usize,
        /// `(degree, coefficient)`，degree 降序。
        terms: Vec<(u32, Number)>,
    },
    /// 多变量 distributed sparse。
    DistributedSparse {
        /// 与 canonical [`Polynomial::terms`] 同形。
        terms: Vec<MonomialTerm>,
    },
}

impl PolynomialRepr {
    /// 从 canonical [`Polynomial`] 转换到指定表示。
    pub fn from_polynomial(poly: &Polynomial, rings: &RingTable, target: ReprTarget) -> Result<Self> {
        let desc = rings.get(poly.ring()).ok_or_else(|| ring_unknown(poly.ring()))?;
        let n = desc.variable_count();
        let body = match target {
            ReprTarget::DistributedSparse => PolynomialReprBody::DistributedSparse { terms: poly.terms().iter().map(|t| t.owning_copy()).collect() },
            ReprTarget::DenseUnivariate { var_index } | ReprTarget::SparseUnivariate { var_index } => {
                if var_index >= n {
                    return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                        .detail("domain", "polynomial")
                        .detail("operation", "repr_var_index_out_of_range"));
                }
                assert_univariate_in(poly.terms(), var_index, n)?;
                if matches!(target, ReprTarget::DenseUnivariate { .. }) {
                    PolynomialReprBody::DenseUnivariate { var_index, coefficients: terms_to_dense(var_index, poly.terms())? }
                }
                else {
                    PolynomialReprBody::SparseUnivariate { var_index, terms: terms_to_sparse(var_index, poly.terms())? }
                }
            }
        };
        Ok(Self { ring: poly.ring(), body })
    }


    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            ring: self.ring,
            body: match &self.body {
                PolynomialReprBody::DenseUnivariate { var_index, coefficients } => PolynomialReprBody::DenseUnivariate {
                    var_index: *var_index,
                    coefficients: clone_numbers(coefficients),
                },
                PolynomialReprBody::SparseUnivariate { var_index, terms } => PolynomialReprBody::SparseUnivariate {
                    var_index: *var_index,
                    terms: terms.iter().map(|(d, c)| (*d, clone_number(c))).collect(),
                },
                PolynomialReprBody::DistributedSparse { terms } => PolynomialReprBody::DistributedSparse {
                    terms: terms.iter().map(|tm| tm.owning_copy()).collect(),
                },
            },
        }
    }

    /// 转回 canonical [`Polynomial`]（merge · 去零 · 排序）。
    pub fn to_polynomial(self, rings: &RingTable) -> Result<Polynomial> {
        let desc = rings.get(self.ring).ok_or_else(|| ring_unknown(self.ring))?;
        let n = desc.variable_count();
        let raw = match self.body {
            PolynomialReprBody::DistributedSparse { terms } => terms,
            PolynomialReprBody::DenseUnivariate { var_index, coefficients } => {
                if var_index >= n {
                    return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                        .detail("domain", "polynomial")
                        .detail("operation", "repr_var_index_out_of_range"));
                }
                dense_to_terms(var_index, n, &coefficients)?
            }
            PolynomialReprBody::SparseUnivariate { var_index, terms } => {
                if var_index >= n {
                    return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                        .detail("domain", "polynomial")
                        .detail("operation", "repr_var_index_out_of_range"));
                }
                sparse_to_terms(var_index, n, &terms)?
            }
        };
        canonicalize_terms(self.ring, desc, raw, rings)
    }

    /// 在同环内切换表示（经 canonical [`Polynomial`] 往返，保证数学相等）。
    pub fn convert(self, rings: &RingTable, target: ReprTarget) -> Result<Self> {
        let poly = self.to_polynomial(rings)?;
        Self::from_polynomial(&poly, rings, target)
    }
}

/// 两种表示经 canonical 化后是否数学相等。
pub fn reprs_mathematically_equal(a: &PolynomialRepr, b: &PolynomialRepr, rings: &RingTable) -> Result<bool> {
    if a.ring != b.ring {
        return Ok(false);
    }
    Ok(a.owning_copy().to_polynomial(rings)? == b.owning_copy().to_polynomial(rings)?)
}

fn assert_univariate_in(terms: &[MonomialTerm], var_index: usize, n: usize) -> Result<()> {
    for term in terms {
        if term.exponents().len() != n {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "repr_exponent_length"));
        }
        for (i, &exp) in term.exponents().iter().enumerate() {
            if i != var_index && exp != 0 {
                return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                    .detail("domain", "polynomial")
                    .detail("operation", "repr_not_univariate"));
            }
        }
    }
    Ok(())
}

fn terms_to_dense(var_index: usize, terms: &[MonomialTerm]) -> Result<Vec<Number>> {
    let mut max_deg = 0usize;
    for term in terms {
        max_deg = max_deg.max(term.exponents()[var_index] as usize);
    }
    let mut coeffs = { let mut __v = Vec::new(); resize_numbers(&mut __v, max_deg + 1, &Number::integer(Integer::zero())); __v };
    for term in terms {
        let d = term.exponents()[var_index] as usize;
        coeffs[d] = clone_number(term.coefficient());
    }
    strip_trailing_zeros(&mut coeffs);
    Ok(coeffs)
}

fn terms_to_sparse(var_index: usize, terms: &[MonomialTerm]) -> Result<Vec<(u32, Number)>> {
    let mut out: Vec<(u32, Number)> = terms.iter().map(|t| (t.exponents()[var_index], clone_number(t.coefficient()))).collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    Ok(out)
}

fn dense_to_terms(var_index: usize, n: usize, coefficients: &[Number]) -> Result<Vec<MonomialTerm>> {
    let mut terms = Vec::new();
    for (d, coeff) in coefficients.iter().enumerate() {
        if coeff.is_zero() {
            continue;
        }
        let mut exponents = vec![0u32; n];
        exponents[var_index] = d as u32;
        terms.push(MonomialTerm { coefficient: clone_number(coeff), exponents });
    }
    Ok(terms)
}

fn sparse_to_terms(var_index: usize, n: usize, terms: &[(u32, Number)]) -> Result<Vec<MonomialTerm>> {
    let mut out = Vec::with_capacity(terms.len());
    for &(deg, ref coeff) in terms {
        if coeff.is_zero() {
            continue;
        }
        let mut exponents = vec![0u32; n];
        exponents[var_index] = deg;
        out.push(MonomialTerm { coefficient: clone_number(coeff), exponents });
    }
    Ok(out)
}

fn strip_trailing_zeros(coeffs: &mut Vec<Number>) {
    while coeffs.last().is_some_and(|c| c.is_zero()) {
        coeffs.pop();
    }
}

fn ring_unknown(ring: RingId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "polynomial")
        .detail("operation", "unknown_ring")
        .detail("ring_id", ring.0.to_string())
}
