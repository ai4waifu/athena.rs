//! Promotion 合同：Integer↔Rational · Exact↔Machine · Machine↔Arbitrary · mismatch。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    decimal::Decimal, domain::NumericDomain, integer::Integer, number::NumericValue, precision::PrecisionKind,
    rational::Rational, real::Real,
};

/// Promotion 策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PromotionPolicy {
    /// 是否允许 Exact → Machine（可能精度损失）。
    pub allow_exact_to_machine: bool,
    /// 是否允许 Arbitrary → Machine。
    pub allow_arbitrary_to_machine: bool,
}

impl Default for PromotionPolicy {
    fn default() -> Self {
        Self { allow_exact_to_machine: false, allow_arbitrary_to_machine: false }
    }
}

/// Promotion 接口。
pub trait Promotion {
    /// 公共域。
    fn common_domain(lhs: &NumericValue, rhs: &NumericValue, policy: &PromotionPolicy) -> Result<NumericDomain, Diagnostic>;

    /// 提升到目标域。
    fn promote(value: NumericValue, target: &NumericDomain, policy: &PromotionPolicy) -> Result<NumericValue, Diagnostic>;
}

/// 默认 promotion。
pub struct DefaultPromotion;

impl Promotion for DefaultPromotion {
    fn common_domain(lhs: &NumericValue, rhs: &NumericValue, policy: &PromotionPolicy) -> Result<NumericDomain, Diagnostic> {
        if lhs.domain() == rhs.domain() {
            return Ok(match (lhs.domain(), lhs.precision().kind, rhs.precision().kind) {
                (NumericDomain::Real, PrecisionKind::Machine, PrecisionKind::Arbitrary)
                | (NumericDomain::Real, PrecisionKind::Arbitrary, PrecisionKind::Machine)
                | (NumericDomain::Real, PrecisionKind::Arbitrary, PrecisionKind::Arbitrary) => NumericDomain::Real,
                _ => lhs.domain(),
            });
        }
        match (lhs.domain(), rhs.domain()) {
            (NumericDomain::Integer, NumericDomain::Rational) | (NumericDomain::Rational, NumericDomain::Integer) => {
                Ok(NumericDomain::Rational)
            }
            (NumericDomain::Integer, NumericDomain::Real)
            | (NumericDomain::Real, NumericDomain::Integer)
            | (NumericDomain::Rational, NumericDomain::Real)
            | (NumericDomain::Real, NumericDomain::Rational) => {
                if policy.allow_exact_to_machine {
                    Ok(NumericDomain::Real)
                }
                else {
                    Err(mismatch("common_domain"))
                }
            }
            _ => Err(mismatch("common_domain")),
        }
    }

    fn promote(value: NumericValue, target: &NumericDomain, policy: &PromotionPolicy) -> Result<NumericValue, Diagnostic> {
        if &value.domain() == target {
            return Ok(value);
        }

        match (value.domain(), target) {
            (NumericDomain::Integer, NumericDomain::Rational) => {
                if let NumericValue::Integer(n) = value {
                    Ok(NumericValue::rational(Rational::from_integer(n)))
                }
                else {
                    Err(failed("promote"))
                }
            }
            (NumericDomain::Rational, NumericDomain::Integer) => {
                if let NumericValue::Rational(r) = value {
                    if r.is_integer() {
                        Ok(NumericValue::integer(r.numerator()))
                    }
                    else {
                        Err(forbidden("rational_to_integer"))
                    }
                }
                else {
                    Err(failed("promote"))
                }
            }
            (NumericDomain::Integer, NumericDomain::Real) => {
                if let NumericValue::Integer(n) = value {
                    exact_to_machine_int(&n, policy)
                }
                else {
                    Err(failed("promote"))
                }
            }
            (NumericDomain::Rational, NumericDomain::Real) => {
                if let NumericValue::Rational(r) = value {
                    exact_to_machine_rat(&r, policy)
                }
                else {
                    Err(failed("promote"))
                }
            }
            (NumericDomain::Real, NumericDomain::Integer) | (NumericDomain::Real, NumericDomain::Rational) => {
                Err(forbidden("real_to_exact"))
            }
            _ => Err(failed("promote")),
        }
    }
}

impl DefaultPromotion {
    /// 同域 Real 精度变更（经 [`Decimal`] 的 Machine ↔ Arbitrary）。
    pub fn promote_real_precision(
        value: NumericValue,
        target_kind: PrecisionKind,
        policy: &PromotionPolicy,
    ) -> Result<NumericValue, Diagnostic> {
        if value.domain() != NumericDomain::Real {
            return Err(mismatch("promote_real_precision"));
        }
        if value.precision().kind == target_kind {
            return Ok(value);
        }
        match (&value, value.precision().kind, target_kind) {
            (NumericValue::Real(Real::Machine(x)), PrecisionKind::Machine, PrecisionKind::Arbitrary) => {
                if !x.is_finite() {
                    return Ok(value);
                }
                let bf = Decimal::from_f64(*x).map_err(|_| forbidden("machine_to_arbitrary"))?;
                Ok(NumericValue::decimal(bf))
            }
            (NumericValue::Real(Real::Decimal(b)), PrecisionKind::Arbitrary, PrecisionKind::Machine) => {
                if !policy.allow_arbitrary_to_machine {
                    return Err(forbidden("arbitrary_to_machine"));
                }
                let x = b.to_f64_round_nearest_even().ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::NumericPrecisionLoss)
                        .detail("domain", "numeric")
                        .detail("operation", "arbitrary_to_machine")
                })?;
                Ok(NumericValue::machine_real(x))
            }
            _ => Err(failed("promote_real_precision")),
        }
    }
}

fn exact_to_machine_int(n: &Integer, policy: &PromotionPolicy) -> Result<NumericValue, Diagnostic> {
    if !policy.allow_exact_to_machine {
        return Err(forbidden("exact_to_machine"));
    }
    match n.try_to_f64_exact() {
        Some(x) => Ok(NumericValue::machine_real(x)),
        None => Err(Diagnostic::new(DiagnosticCode::NumericPrecisionLoss)
            .detail("domain", "numeric")
            .detail("operation", "integer_to_machine")),
    }
}

fn exact_to_machine_rat(r: &Rational, policy: &PromotionPolicy) -> Result<NumericValue, Diagnostic> {
    if !policy.allow_exact_to_machine {
        return Err(forbidden("exact_to_machine"));
    }
    match r.try_to_f64_exact() {
        Some(x) => Ok(NumericValue::machine_real(x)),
        None => Err(Diagnostic::new(DiagnosticCode::NumericPrecisionLoss)
            .detail("domain", "numeric")
            .detail("operation", "rational_to_machine")),
    }
}

fn mismatch(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "numeric").detail("operation", op)
}

fn failed(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericPromotionFailed).detail("domain", "numeric").detail("operation", op)
}

fn forbidden(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", op)
}
