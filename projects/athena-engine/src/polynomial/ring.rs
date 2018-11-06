//! 多项式环描述符与系数域合同。

use athena_numeric::{Integer, Modulus};
use athena_types::{Diagnostic, DiagnosticCode, FieldId, RingId, SymbolId};

use crate::number_theory::{Primality, primality_test};

use super::{fingerprint::RingFingerprint, order::MonomialOrder};

/// 精确 / 近似系数域（环身份的一部分）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoefficientDomain {
    /// ℤ（特征 0）。
    Integer,
    /// ℚ（特征 0）。
    Rational,
    /// 素域 𝔽_p（`p` 须为已验证素数）。
    ///
    /// **Deprecation（Living `18`）**：intern 时规范化为 [`CoefficientDomain::FiniteField`]。
    PrimeField {
        /// 特征素数。
        p: Integer,
    },
    /// ℤ/nℤ（精确但一般非域）。
    ModularInteger {
        /// 模数 `n > 1`。
        modulus: Modulus,
    },
    /// 已注册有限域（经 [`FieldId`]）。
    FiniteField {
        /// 域句柄。
        field: FieldId,
        /// 域特征 `p`（须与 [`FieldTable`] presentation 一致）。
        characteristic: Integer,
    },
    /// 机器 / 近似实数 — 不得进入精确多项式环。
    ApproximateReal,
}

/// 环特征。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RingCharacteristic {
    /// 特征 0（ℤ、ℚ 等）。
    Zero,
    /// 特征 `n > 0`（素域、ℤ/nℤ、有限域等）。
    Positive(Integer),
}

/// 多项式环完整描述符（`RingId` 的内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingDescriptor {
    /// 稳定环 id。
    pub id: RingId,
    /// 系数环 intern 句柄。
    pub coefficient_ring: athena_types::CoefficientRingId,
    /// 系数域。
    pub coefficients: CoefficientDomain,
    /// 有序、无重复的变量身份。
    pub variables: Vec<SymbolId>,
    /// 单项式序（环身份，非算法选项）。
    pub order: MonomialOrder,
    /// 环特征（由系数域推导）。
    pub characteristic: RingCharacteristic,
    /// 稳定数学身份摘要（不含 Session 句柄）。
    pub ring_fingerprint: RingFingerprint,
}

impl RingDescriptor {
    /// 校验环内容（不含 `RingId`；由 [`super::ring_table::RingTable`] 分配 id）。
    pub fn validate_content(
        coefficients: CoefficientDomain,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
    ) -> Result<(CoefficientDomain, Vec<SymbolId>, MonomialOrder, RingCharacteristic), Diagnostic> {
        if matches!(coefficients, CoefficientDomain::ApproximateReal) {
            return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
                .detail("domain", "polynomial")
                .detail("operation", "approximate_coefficient_domain"));
        }
        if has_duplicate_symbols(&variables) {
            return Err(Diagnostic::new(DiagnosticCode::PolynomialVariableMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "duplicate_variable"));
        }
        order.validate_for_variables(variables.len())?;
        let coefficients = validate_coefficient_domain(coefficients)?;
        let characteristic = characteristic_of(&coefficients)?;
        Ok((coefficients, variables, order, characteristic))
    }

    /// 构造带 id 的描述符（intern 后使用）。
    pub(crate) fn with_id(
        id: RingId,
        coefficient_ring: athena_types::CoefficientRingId,
        coefficients: CoefficientDomain,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
        characteristic: RingCharacteristic,
        ring_fingerprint: RingFingerprint,
    ) -> Self {
        Self { id, coefficient_ring, coefficients, variables, order, characteristic, ring_fingerprint }
    }

    /// 变量数。
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }
}

/// 显式除法策略 — `ℤ[x]` 不得无条件域除。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DivisionPolicy {
    /// 仅精确整除。
    #[default]
    ExactOnly,
    /// 系数域上的域除法。
    FieldDivision,
    /// 伪除。
    PseudoDivision,
    /// 提升到有理系数（须写入 metadata）。
    PromoteToRational,
}

fn has_duplicate_symbols(vars: &[SymbolId]) -> bool {
    let mut seen = vars.to_vec();
    seen.sort_by_key(|s| s.0);
    seen.windows(2).any(|w| w[0] == w[1])
}

fn validate_coefficient_domain(coefficients: CoefficientDomain) -> Result<CoefficientDomain, Diagnostic> {
    validate_coefficient_domain_public(coefficients)
}

/// 校验并规范化系数域（系数环 intern 与多项式环构造共用）。
pub(crate) fn validate_coefficient_domain_public(coefficients: CoefficientDomain) -> Result<CoefficientDomain, Diagnostic> {
    match coefficients {
        CoefficientDomain::PrimeField { p } => {
            validate_prime_modulus(&p)?;
            Ok(CoefficientDomain::PrimeField { p })
        }
        CoefficientDomain::FiniteField { field, characteristic } => {
            if characteristic.is_zero() || characteristic.is_negative() {
                return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                    .detail("domain", "polynomial")
                    .detail("operation", "finite_field_characteristic"));
            }
            let _ = field;
            Ok(CoefficientDomain::FiniteField { field, characteristic })
        }
        other => Ok(other),
    }
}

/// 校验素数模（多项式环构造共用）。
pub(crate) fn validate_prime_modulus(p: &Integer) -> Result<(), Diagnostic> {
    if p.is_zero() || p.is_negative() {
        return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
            .detail("domain", "polynomial")
            .detail("operation", "prime_field_characteristic"));
    }
    match primality_test(p, None) {
        Primality::Prime => Ok(()),
        Primality::Composite => Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
            .detail("domain", "polynomial")
            .detail("operation", "prime_field_not_prime")),
        Primality::ProbablePrime { .. } | Primality::Unknown => Err(Diagnostic::new(DiagnosticCode::PrimeTestInconclusive)
            .detail("domain", "polynomial")
            .detail("operation", "prime_field_characteristic")),
    }
}

pub(crate) fn characteristic_of(coeff: &CoefficientDomain) -> Result<RingCharacteristic, Diagnostic> {
    match coeff {
        CoefficientDomain::Integer | CoefficientDomain::Rational => Ok(RingCharacteristic::Zero),
        CoefficientDomain::ModularInteger { modulus } => Ok(RingCharacteristic::Positive(modulus.value().clone())),
        CoefficientDomain::PrimeField { p } => Ok(RingCharacteristic::Positive(p.clone())),
        CoefficientDomain::FiniteField { characteristic, .. } => Ok(RingCharacteristic::Positive(characteristic.clone())),
        CoefficientDomain::ApproximateReal => unreachable!("validated earlier"),
    }
}
