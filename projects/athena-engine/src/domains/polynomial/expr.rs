//! 稀疏多项式对象：仅经 Builder / canonicalize 进入算法层。

use athena_numeric::Number;
use athena_types::RingId;

use crate::runtime::values::numeric_clone::clone_number;

/// 单项式项：系数 × 指数向量（与环变量表对齐）。
///
/// 字段私有：禁止外部构造出零系数、错误宽度或未校验的项。
#[derive(Debug, PartialEq)]
pub struct MonomialTerm {
    pub(crate) coefficient: Number,
    pub(crate) exponents: Vec<u32>,
}

impl MonomialTerm {
    /// 受信任构造（调用方已保证系数非零、指数宽度合法）。
    pub(crate) fn from_parts(coefficient: Number, exponents: Vec<u32>) -> Self {
        Self { coefficient, exponents }
    }

    /// 系数。
    pub fn coefficient(&self) -> &Number {
        &self.coefficient
    }

    /// 指数向量。
    pub fn exponents(&self) -> &[u32] {
        &self.exponents
    }

    /// 拆出所有权部件（算法内部合并 / 移动用）。
    pub(crate) fn into_parts(self) -> (Number, Vec<u32>) {
        (self.coefficient, self.exponents)
    }

    /// 克隆指数向量（避免暴露可变 `&mut Vec`）。
    pub(crate) fn exponents_vec(&self) -> Vec<u32> {
        self.exponents.clone()
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { coefficient: clone_number(&self.coefficient), exponents: self.exponents.clone() }
    }
}

impl Clone for MonomialTerm {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}

/// 规范多项式：同环、非零项、每单项式至多一项、按环序排序。
///
/// 仅 [`super::PolynomialBuilder`]、[`super::canonicalize_polynomial`] 与 crate 内受信任路径可构造。
#[derive(Debug, PartialEq)]
pub struct CanonicalPolynomial {
    pub(crate) ring: RingId,
    pub(crate) terms: Vec<MonomialTerm>,
}

/// 迁移别名：公共合同与算法层均指 [`CanonicalPolynomial`]。
pub type Polynomial = CanonicalPolynomial;

impl CanonicalPolynomial {
    /// 零多项式。
    pub fn zero(ring: RingId) -> Self {
        Self { ring, terms: Vec::new() }
    }

    /// 受信任构造：`terms` 必须已满足 canonical 不变量。
    pub(crate) fn from_canonical_parts(ring: RingId, terms: Vec<MonomialTerm>) -> Self {
        Self { ring, terms }
    }

    /// 所属环。
    pub fn ring(&self) -> RingId {
        self.ring
    }

    /// 规范项列表（只读）。
    pub fn terms(&self) -> &[MonomialTerm] {
        &self.terms
    }

    /// 是否为零多项式。
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// 拆出所有权部件（canonicalize 再入、表示转换）。
    pub(crate) fn into_parts(self) -> (RingId, Vec<MonomialTerm>) {
        (self.ring, self.terms)
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { ring: self.ring, terms: self.terms.iter().map(MonomialTerm::owning_copy).collect() }
    }
}

impl Clone for CanonicalPolynomial {
    fn clone(&self) -> Self {
        self.owning_copy()
    }
}
