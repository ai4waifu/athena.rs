//! 数值证书。

use crate::{interval::Interval, real::Real};

/// 证书方法。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CertificateMethod {
    /// 未指定。
    #[default]
    Unspecified,
    /// 区间算术。
    IntervalArithmetic,
    /// 任意精度余项。
    ArbitraryRemainder,
}

/// 数值证书。
#[derive(Debug, Clone, PartialEq)]
pub struct NumericCertificate {
    /// 绝对误差上界。
    pub absolute_error: Option<Real>,
    /// 相对误差上界。
    pub relative_error: Option<Real>,
    /// enclosure。
    pub enclosure: Option<Interval>,
    /// 方法。
    pub method: CertificateMethod,
}
