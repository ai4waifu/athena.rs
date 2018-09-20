//! 比较语义（N1：Integer / Rational；跨域先 promotion）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    number::{NumericRepr, NumericValue},
    promotion::{DefaultPromotion, Promotion, PromotionPolicy},
};

/// 比较结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericComparison {
    /// 同一表示。
    Identity,
    /// 精确相等。
    ExactEqual,
    /// 已证明相等。
    ProvenEqual,
    /// 区间重叠。
    IntervalOverlap,
    /// 近似相等。
    ApproximateEqual,
    /// 不等。
    Unequal,
    /// 未知。
    Unknown,
}

/// 比较策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ComparisonPolicy {
    /// 是否允许近似。
    pub allow_approximate: bool,
    /// 跨 Integer/Rational 时使用的 promotion 策略。
    pub promotion: PromotionPolicy,
}

/// 比较接口。
pub trait NumericCompare {
    /// 比较两值。
    fn compare(lhs: &NumericValue, rhs: &NumericValue, policy: &ComparisonPolicy) -> Result<NumericComparison, Diagnostic>;
}

/// 默认比较器。
pub struct DefaultNumericCompare;

impl NumericCompare for DefaultNumericCompare {
    fn compare(lhs: &NumericValue, rhs: &NumericValue, policy: &ComparisonPolicy) -> Result<NumericComparison, Diagnostic> {
        if std::ptr::eq(lhs, rhs) {
            return Ok(NumericComparison::Identity);
        }
        if lhs.domain() != rhs.domain() {
            let domain = DefaultPromotion::common_domain(lhs, rhs, &policy.promotion)?;
            let a = DefaultPromotion::promote(lhs.clone(), &domain, &policy.promotion)?;
            let b = DefaultPromotion::promote(rhs.clone(), &domain, &policy.promotion)?;
            return Self::compare_same_domain(&a, &b, policy);
        }
        Self::compare_same_domain(lhs, rhs, policy)
    }
}

impl DefaultNumericCompare {
    fn compare_same_domain(
        lhs: &NumericValue,
        rhs: &NumericValue,
        _policy: &ComparisonPolicy,
    ) -> Result<NumericComparison, Diagnostic> {
        match (lhs.repr(), rhs.repr()) {
            (NumericRepr::Integer(a), NumericRepr::Integer(b)) => {
                Ok(if a == b { NumericComparison::ExactEqual } else { NumericComparison::Unequal })
            }
            (NumericRepr::Rational(a), NumericRepr::Rational(b)) => {
                Ok(if a == b { NumericComparison::ExactEqual } else { NumericComparison::Unequal })
            }
            (NumericRepr::Real(a), NumericRepr::Real(b)) if _policy.allow_approximate => {
                use crate::real::Real;
                match (a, b) {
                    (Real::Machine(x), Real::Machine(y)) if x.is_finite() && y.is_finite() => {
                        let tol = 1e-12 * (1.0 + x.abs() + y.abs());
                        Ok(if (x - y).abs() <= tol { NumericComparison::ApproximateEqual } else { NumericComparison::Unequal })
                    }
                    (Real::Arbitrary { ieee754_bits: x, .. }, Real::Arbitrary { ieee754_bits: y, .. }) => {
                        Ok(if x == y { NumericComparison::ExactEqual } else { NumericComparison::Unequal })
                    }
                    _ => Ok(NumericComparison::Unknown),
                }
            }
            (NumericRepr::Real(_), NumericRepr::Real(_)) => Ok(NumericComparison::Unknown),
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "compare")),
        }
    }
}
