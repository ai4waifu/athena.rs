//! [`PolynomialMatrixView`] — 多项式 → 稀疏矩阵风格投影（零拷贝脚手架）。
//!
//! 完整 Macaulay 展开 / CSR 物化属后续切片。本视图只借用源多项式项，
//! 禁止 `Vec` 复制成独立矩阵 DomainObject。

use athena_types::RingId;

use super::{TypedViewHeader, ViewFingerprint, ViewKind, ViewRevision};
use crate::domains::polynomial::{MonomialTerm, Polynomial, PolynomialObjectStore, PolynomialRef};

/// 只读多项式矩阵投影：借用 [`Polynomial::terms`]，不物化 Macaulay 矩阵。
#[derive(Debug, Clone, Copy)]
pub struct PolynomialMatrixView<'a> {
    header: TypedViewHeader,
    poly: &'a Polynomial,
}

impl<'a> PolynomialMatrixView<'a> {
    /// 从 Session 多项式仓打开视图；缺失 handle 时返回 `None`。
    pub fn open(store: &'a PolynomialObjectStore, poly_ref: PolynomialRef) -> Option<Self> {
        let poly = store.get(poly_ref)?;
        let source = store.object_ref(poly_ref)?;
        let fingerprint = ViewFingerprint(store.fingerprint(poly_ref)?.0);
        let header = TypedViewHeader::new(source, ViewKind::PolynomialMatrix, ViewRevision(0), fingerprint);
        Some(Self { header, poly })
    }

    /// 公共头。
    pub const fn header(&self) -> TypedViewHeader {
        self.header
    }

    /// 源多项式所属环。
    pub fn ring(&self) -> RingId {
        self.poly.ring()
    }

    /// 是否为零多项式。
    pub fn is_zero(&self) -> bool {
        self.poly.is_zero()
    }

    /// 稀疏单项式项（借用；禁止 `collect` 成 owning 跨域矩阵载荷）。
    pub fn terms(&self) -> &'a [MonomialTerm] {
        self.poly.terms()
    }

    /// 非零项数（稀疏 nnz 上界，非 Macaulay 列维）。
    pub fn nnz(&self) -> usize {
        self.poly.terms().len()
    }
}
