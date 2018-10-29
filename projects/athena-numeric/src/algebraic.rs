//! 代数数骨架。

use crate::interval::Interval;
use crate::polynomial_fingerprint::PolynomialFingerprint;

/// 代数数表示策略。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlgebraicRepresentation {
    /// 极小多项式 + 隔离区间。
    MinimalPolynomial {
        /// 稳定极小多项式指纹（非 Session 局部 IR）。
        polynomial: PolynomialFingerprint,
        /// 根下标（相对明确根排序与隔离证书）。
        root_index: u32,
    },
    /// 占位。
    Placeholder,
}

/// 代数数。
#[derive(Debug, Clone, PartialEq)]
pub struct AlgebraicNumber {
    /// 极小多项式指纹。
    pub minimal_polynomial: PolynomialFingerprint,
    /// 实隔离区间（非实代数数须用复隔离区域，后续）。
    pub isolating_interval: Interval,
    /// 表示。
    pub representation: AlgebraicRepresentation,
}
