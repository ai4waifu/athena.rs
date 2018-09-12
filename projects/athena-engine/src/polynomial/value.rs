//! 多项式域对外值句柄。

use super::expr::Polynomial;

/// 擦除后的多项式句柄（不暴露泛型多项式）。
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialValue {
    /// 内部多项式对象。
    pub inner: Polynomial,
}

/// 多项式域返回值。
#[derive(Debug, Clone, PartialEq)]
pub enum PolynomialDomainValue {
    /// 单个多项式。
    Polynomial(PolynomialValue),
    /// 占位：后续 GCD / 因式列表等。
    Placeholder,
}
