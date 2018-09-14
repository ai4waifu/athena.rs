//! 比较语义（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::number::NumericValue;

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
}

/// 比较接口。
pub trait NumericCompare {
    /// 比较两值。
    fn compare(
        lhs: &NumericValue,
        rhs: &NumericValue,
        policy: &ComparisonPolicy,
    ) -> Result<NumericComparison, Diagnostic>;
}

/// 默认比较器（骨架：仅同域整数相等）。
pub struct DefaultNumericCompare;

impl NumericCompare for DefaultNumericCompare {
    fn compare(
        lhs: &NumericValue,
        rhs: &NumericValue,
        _policy: &ComparisonPolicy,
    ) -> Result<NumericComparison, Diagnostic> {
        if lhs.domain != rhs.domain {
            return Ok(NumericComparison::Unknown);
        }
        match (&lhs.value, &rhs.value) {
            (crate::number::NumericRepr::Integer(a), crate::number::NumericRepr::Integer(b)) if a == b => {
                Ok(NumericComparison::ExactEqual)
            }
            (crate::number::NumericRepr::Integer(_), crate::number::NumericRepr::Integer(_)) => {
                Ok(NumericComparison::Unequal)
            }
            _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "compare")),
        }
    }
}
