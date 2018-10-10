//! 可变多项式构造器 — 唯一公开入口，产出 canonical [`Polynomial`]。

use athena_numeric::Number;
use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    canonical::canonicalize_terms,
    expr::{MonomialTerm, Polynomial},
    ring_table::RingTable,
};

/// 可变构造器（非 canonical；仅 [`Self::build`] 产出规范多项式）。
#[derive(Debug, Clone)]
pub struct PolynomialBuilder {
    ring: RingId,
    terms: Vec<MonomialTerm>,
}

impl PolynomialBuilder {
    /// 空构造器（零多项式骨架）。
    pub fn new(ring: RingId) -> Self {
        Self { ring, terms: Vec::new() }
    }

    /// 追加一项（允许重复单项式与零系数；[`Self::build`] 时合并/剔除）。
    pub fn push_term(&mut self, coefficient: Number, exponents: Vec<u32>) -> Result<()> {
        self.terms.push(MonomialTerm { coefficient, exponents });
        Ok(())
    }

    /// 校验、合并同类项、按环单项式序排序，产出 canonical [`Polynomial`]。
    pub fn build(self, rings: &RingTable) -> Result<Polynomial> {
        let desc = rings.get(self.ring).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "polynomial")
                .detail("operation", "unknown_ring")
                .detail("ring_id", self.ring.0.to_string())
        })?;
        canonicalize_terms(self.ring, desc, self.terms)
    }
}

/// 规范多项式别名（与 [`Polynomial`] 相同；仅经 [`PolynomialBuilder`] 或 [`super::canonicalize_polynomial`] 构造）。
pub type CanonicalPolynomial = Polynomial;
