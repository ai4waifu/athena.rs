//! [`SeriesPolynomialView`] — 级数 → 多项式风格系数投影（零拷贝）。

use athena_types::{SymbolId, TermId};

use super::{TypedViewHeader, ViewFingerprint, ViewKind, ViewRevision};
use crate::domains::calculus::{Remainder, Series, SeriesObjectStore, SeriesRef};

/// 只读级数多项式投影：借用 [`Series::terms`]，不复制系数向量。
#[derive(Debug, Clone, Copy)]
pub struct SeriesPolynomialView<'a> {
    header: TypedViewHeader,
    series: &'a Series,
}

impl<'a> SeriesPolynomialView<'a> {
    /// 从 Session 级数仓打开视图；缺失 handle 时返回 `None`。
    pub fn open(store: &'a SeriesObjectStore, series_ref: SeriesRef) -> Option<Self> {
        let series = store.get(series_ref)?;
        let source = store.object_ref(series_ref)?;
        let fingerprint = ViewFingerprint(store.fingerprint(series_ref)?);
        let header = TypedViewHeader::new(source, ViewKind::SeriesPolynomial, ViewRevision(0), fingerprint);
        Some(Self { header, series })
    }

    /// 公共头。
    pub const fn header(&self) -> TypedViewHeader {
        self.header
    }

    /// 展开变量。
    pub const fn variable(&self) -> SymbolId {
        self.series.variable
    }

    /// 展开中心。
    pub const fn center(&self) -> TermId {
        self.series.center
    }

    /// 截断阶。
    pub const fn order(&self) -> u32 {
        self.series.order
    }

    /// 余项（借用）。
    pub const fn remainder(&self) -> &Remainder {
        &self.series.remainder
    }

    /// 系数/幂次项（借用，禁止调用方 `collect` 成 owning 跨域载荷）。
    pub fn terms(&self) -> &'a [(TermId, i64)] {
        self.series.terms.as_slice()
    }

    /// 项数。
    pub fn term_count(&self) -> usize {
        self.series.terms.len()
    }
}
