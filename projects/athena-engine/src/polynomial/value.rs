//! 多项式域对外值句柄。

use super::{
    certificate::{GroebnerCertificate, GroebnerStatus},
    expr::Polynomial,
    factor::PolynomialFactorization,
    groebner::GroebnerComputation,
};
use athena_types::RingId;

/// 擦除后的多项式句柄（不暴露泛型多项式）。
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialValue {
    /// 内部多项式对象。
    pub inner: Polynomial,
}

/// Gröbner / 消元基结果。
#[derive(Debug, Clone, PartialEq)]
pub struct GroebnerBasisValue {
    /// 所属环。
    pub ring: RingId,
    /// 基或候选多项式（canonical）。
    pub basis: Vec<Polynomial>,
    /// 计算证书。
    pub certificate: GroebnerCertificate,
    /// 显式状态分型（M-Graph admission 只接纳 [`GroebnerStatus::Verified`]）。
    pub status: GroebnerStatus,
}

impl GroebnerBasisValue {
    /// 从 [`GroebnerComputation`] 构造域值。
    pub fn from_computation(computation: GroebnerComputation) -> Self {
        let status = computation.status();
        let ring = computation.ring();
        let certificate = computation.certificate().clone();
        let basis = computation.polynomials().to_vec();
        Self { ring, basis, certificate, status }
    }

    /// 是否可作为 exact witness。
    pub fn is_exact_witness(&self) -> bool {
        self.status == GroebnerStatus::Verified && self.certificate.is_exact_witness()
    }
}

/// 单变量除法结果值。
#[derive(Debug, Clone, PartialEq)]
pub struct UnivariateDivisionValue {
    /// 商。
    pub quotient: PolynomialValue,
    /// 余式。
    pub remainder: PolynomialValue,
}

/// 多项式域返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialDomainValue {
    /// 单个多项式。
    Polynomial(PolynomialValue),
    /// 单变量除法（商 + 余式）。
    UnivariateDivision(UnivariateDivisionValue),
    /// 因式分解（带完备性分型）。
    Factorization(PolynomialFactorization),
    /// Gröbner / 消元基。
    GroebnerBasis(GroebnerBasisValue),
    /// 占位。
    Placeholder,
}
