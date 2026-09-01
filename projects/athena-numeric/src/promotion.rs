//! Promotion 合同（N2：Integer↔Rational · Exact↔Machine · Machine↔Arbitrary · mismatch）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domain::NumericDomain,
    integer::Integer,
    number::{NumericRepr, NumericValue},
    precision::{PrecisionInfo, PrecisionKind},
    rational::Rational,
    real::Real,
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
        if lhs.domain == rhs.domain {
            return Ok(match (&lhs.domain, &lhs.precision.kind, &rhs.precision.kind) {
                (NumericDomain::Real, PrecisionKind::Machine, PrecisionKind::Arbitrary)
                | (NumericDomain::Real, PrecisionKind::Arbitrary, PrecisionKind::Machine)
                | (NumericDomain::Real, PrecisionKind::Arbitrary, PrecisionKind::Arbitrary) => NumericDomain::Real,
                _ => lhs.domain.clone(),
            });
        }
        match (&lhs.domain, &rhs.domain) {
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
        if &value.domain == target {
            return match (target, value.precision.kind) {
                (NumericDomain::Real, PrecisionKind::Machine) => {
                    // 同域 Machine → Arbitrary 由调用方传 Arbitrary 精度目标时走下方分支；此处恒等。
                    Ok(value)
                }
                _ => Ok(value),
            };
        }

        match (&value.domain, target, &value.value) {
            (NumericDomain::Integer, NumericDomain::Rational, NumericRepr::Integer(n)) => {
                Ok(NumericValue::rational(Rational::from_integer(n.clone())))
            }
            (NumericDomain::Rational, NumericDomain::Integer, NumericRepr::Rational(r)) if r.is_integer() => {
                Ok(NumericValue::integer(r.numerator()))
            }
            (NumericDomain::Rational, NumericDomain::Integer, _) => Err(forbidden("rational_to_integer")),
            (NumericDomain::Integer, NumericDomain::Real, NumericRepr::Integer(n)) => exact_to_machine_int(n, policy),
            (NumericDomain::Rational, NumericDomain::Real, NumericRepr::Rational(r)) => exact_to_machine_rat(r, policy),
            (NumericDomain::Real, NumericDomain::Integer, _) | (NumericDomain::Real, NumericDomain::Rational, _) => {
                Err(forbidden("real_to_exact"))
            }
            _ => Err(failed("promote")),
        }
    }
}

impl DefaultPromotion {
    /// 同域 Real：Machine ↔ Arbitrary。
    pub fn promote_real_precision(
        value: NumericValue,
        target_kind: PrecisionKind,
        policy: &PromotionPolicy,
    ) -> Result<NumericValue, Diagnostic> {
        if value.domain != NumericDomain::Real {
            return Err(mismatch("promote_real_precision"));
        }
        if value.precision.kind == target_kind {
            return Ok(value);
        }
        match (&value.value, value.precision.kind, target_kind) {
            (NumericRepr::Real(Real::Machine(x)), PrecisionKind::Machine, PrecisionKind::Arbitrary) => Ok(NumericValue {
                domain: NumericDomain::Real,
                value: NumericRepr::Real(Real::Arbitrary { decimal: format!("{x}"), precision: PrecisionInfo::arbitrary(53) }),
                precision: PrecisionInfo::arbitrary(53),
                provenance: value.provenance,
            }),
            (NumericRepr::Real(Real::Arbitrary { decimal, .. }), PrecisionKind::Arbitrary, PrecisionKind::Machine) => {
                if !policy.allow_arbitrary_to_machine {
                    return Err(forbidden("arbitrary_to_machine"));
                }
                let x: f64 = decimal.parse().map_err(|_| {
                    Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                        .detail("domain", "numeric")
                        .detail("operation", "arbitrary_parse")
                })?;
                if !x.is_finite() {
                    return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                        .detail("domain", "numeric")
                        .detail("operation", "arbitrary_non_finite"));
                }
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
    match n.to_f64_exact_machine() {
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
    match r.to_f64_exact_machine() {
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
