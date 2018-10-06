//! 稀疏多项式对象（F1 前项未强制 canonical；环身份经 [`RingId`]）。

use athena_numeric::Number;
use athena_types::RingId;

/// 单项式项：系数 × 指数向量（与环变量表对齐）。
#[derive(Debug, Clone, PartialEq)]
pub struct MonomialTerm {
    /// 系数（零系数不得保留；公开构造经 F1 Builder）。
    pub coefficient: Number,
    /// 各变量指数（长度须等于环变量数）。
    pub exponents: Vec<u32>,
}

/// 多项式 = 环 id + 稀疏项。
#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial {
    /// 所属环（系数域 + 变量 + 单项式序）。
    pub ring: RingId,
    /// 按环单项式序排序的非零项（F1 强制 canonical）。
    pub terms: Vec<MonomialTerm>,
}

impl Polynomial {
    /// 零多项式。
    pub fn zero(ring: RingId) -> Self {
        Self { ring, terms: Vec::new() }
    }
}
