//! Promotion 合同（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{domain::NumericDomain, number::NumericValue};

/// Promotion 策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PromotionPolicy {
    /// 是否允许 Exact → Machine。
    pub allow_exact_to_machine: bool,
}

/// Promotion 接口。
pub trait Promotion {
    /// 公共域。
    fn common_domain(
        lhs: &NumericValue,
        rhs: &NumericValue,
        policy: &PromotionPolicy,
    ) -> Result<NumericDomain, Diagnostic>;

    /// 提升到目标域。
    fn promote(
        value: NumericValue,
        target: &NumericDomain,
        policy: &PromotionPolicy,
    ) -> Result<NumericValue, Diagnostic>;
}

/// 默认 promotion（骨架：仅同域或 Integer→Rational）。
pub struct DefaultPromotion;

impl Promotion for DefaultPromotion {
    fn common_domain(
        lhs: &NumericValue,
        rhs: &NumericValue,
        _policy: &PromotionPolicy,
    ) -> Result<NumericDomain, Diagnostic> {
        if lhs.domain == rhs.domain {
            return Ok(lhs.domain.clone());
        }
        match (&lhs.domain, &rhs.domain) {
            (NumericDomain::Integer, NumericDomain::Rational)
            | (NumericDomain::Rational, NumericDomain::Integer) => Ok(NumericDomain::Rational),
            _ => Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "common_domain")),
        }
    }

    fn promote(
        value: NumericValue,
        target: &NumericDomain,
        _policy: &PromotionPolicy,
    ) -> Result<NumericValue, Diagnostic> {
        if &value.domain == target {
            return Ok(value);
        }
        Err(Diagnostic::new(DiagnosticCode::PromotionFailed)
            .detail("domain", "numeric")
            .detail("operation", "promote"))
    }
}
