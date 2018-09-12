//! 稀疏多项式对象（规范化后才可 hash）。

use athena_types::Number;

use super::ring::CoefficientRing;

/// 单项式项：系数 × 指数向量（与变量表对齐）。
#[derive(Debug, Clone, PartialEq)]
pub struct MonomialTerm {
    /// 系数（零系数不得保留）。
    pub coefficient: Number,
    /// 各变量指数。
    pub exponents: Vec<u32>,
}

/// 多项式 = 环 + 变量 + 稀疏项。
#[derive(Debug, Clone, PartialEq)]
pub struct Polynomial {
    /// 系数环。
    pub ring: CoefficientRing,
    /// 变量名（桥接；后续可换 `SymbolId`）。
    pub variables: Vec<String>,
    /// 按 monomial order 排序的非零项。
    pub terms: Vec<MonomialTerm>,
}

impl Polynomial {
    /// 空多项式（零）。
    pub fn zero(ring: CoefficientRing, variables: Vec<String>) -> Self {
        Self { ring, variables, terms: Vec::new() }
    }
}
