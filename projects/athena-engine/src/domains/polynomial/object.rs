//! 稀疏多项式对象：仅经 Builder / canonicalize 进入算法层。

use athena_numeric::Number;
use athena_types::RingId;

use crate::runtime::values::numeric_clone::clone_number;

/// 单项式项：系数 × 指数向量（与环变量表对齐）。
///
/// 字段私有：禁止外部构造出零系数、错误宽度或未校验的项。
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
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

    /// Owning 复制（显式深复制，禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self { coefficient: clone_number(&self.coefficient), exponents: self.exponents.clone() }
    }
}

/// 规范多项式对象身份。
///
/// 不变量：同环、非零项、每单项式至多一项、按环单项式序排序。
/// 仅 [`super::PolynomialBuilder`]、[`super::canonicalize_polynomial`] 与 crate 内受信任路径可构造。
/// 这是多项式域实体，不是 Athena `Term`，也不是方言 Expression。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]（后续应改为目标 heap 的显式 GC clone）。
#[derive(Debug, PartialEq)]
pub struct CanonicalPolynomial {
    pub(crate) ring: RingId,
    pub(crate) terms: Vec<MonomialTerm>,
}

/// 算法调用点短名：与 [`CanonicalPolynomial`] 同一类型。
pub use CanonicalPolynomial as Polynomial;

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

    /// Owning 复制（显式深复制，禁止默认 `Clone`）。
    pub fn owning_copy(&self) -> Self {
        Self { ring: self.ring, terms: self.terms.iter().map(MonomialTerm::owning_copy).collect() }
    }
}
