//! 多项式域对外值句柄。

use super::{
    certificate::GroebnerCertificate,
    expr::Polynomial,
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
    /// 基多项式（canonical）。
    pub basis: Vec<Polynomial>,
    /// 计算证书。
    pub certificate: GroebnerCertificate,
}

/// 多项式域返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialDomainValue {
    /// 单个多项式。
    Polynomial(PolynomialValue),
    /// Gröbner / 消元基。
    GroebnerBasis(GroebnerBasisValue),
    /// 占位：后续 GCD / 因式列表等。
    Placeholder,
}
