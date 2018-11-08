//! 多项式环描述符与系数域合同。

use athena_numeric::{Integer, Modulus};
use athena_types::{Diagnostic, DiagnosticCode, FieldId, SymbolId};

use crate::algebra::{CoefficientParent, FieldTable};

use super::{fingerprint::RingFingerprint, order::MonomialOrder};

/// 精确 / 近似系数域（系数环 intern 键；多项式环身份见 [`CoefficientParent`]）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoefficientDomain {
    /// ℤ（特征 0）。
    Integer,
    /// ℚ（特征 0）。
    Rational,
    /// ℤ/nℤ（精确但一般非域）。
    ModularInteger {
        /// 模数 `n > 1`。
        modulus: Modulus,
    },
    /// 已注册有限域（经 [`FieldId`]；特征与约化模数由 [`FieldTable`] presentation 提供）。
    FiniteField {
        /// 域句柄。
        field: FieldId,
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
    pub id: athena_types::RingId,
    /// 系数环 intern 句柄。
    pub coefficient_ring: athena_types::CoefficientRingId,
    /// 系数父对象（Living `18` Phase 2 真相源）。
    pub coefficients: CoefficientParent,
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
        fields: &FieldTable,
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
        let coefficients = validate_coefficient_domain(coefficients, fields)?;
        let characteristic = characteristic_of(&coefficients, fields)?;
        Ok((coefficients, variables, order, characteristic))
    }

    /// 构造带 id 的描述符（intern 后使用）。
    pub(crate) fn with_id(
        id: athena_types::RingId,
        coefficient_ring: athena_types::CoefficientRingId,
        coefficients: CoefficientParent,
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

fn validate_coefficient_domain(coefficients: CoefficientDomain, fields: &FieldTable) -> Result<CoefficientDomain, Diagnostic> {
    validate_coefficient_domain_public(coefficients, fields)
}

/// 校验并规范化系数域（系数环 intern 与多项式环构造共用）。
pub(crate) fn validate_coefficient_domain_public(
    coefficients: CoefficientDomain,
    fields: &FieldTable,
) -> Result<CoefficientDomain, Diagnostic> {
    match coefficients {
        CoefficientDomain::FiniteField { field } => {
            fields.validate_finite_field(field)?;
            Ok(CoefficientDomain::FiniteField { field })
        }
        other => Ok(other),
    }
}

pub(crate) fn characteristic_of(coeff: &CoefficientDomain, fields: &FieldTable) -> Result<RingCharacteristic, Diagnostic> {
    match coeff {
        CoefficientDomain::Integer | CoefficientDomain::Rational => Ok(RingCharacteristic::Zero),
        CoefficientDomain::ModularInteger { modulus } => Ok(RingCharacteristic::Positive(modulus.value().clone())),
        CoefficientDomain::FiniteField { field } => {
            let p = fields.characteristic(*field).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "polynomial")
                    .detail("operation", "finite_field_characteristic")
            })?;
            Ok(RingCharacteristic::Positive(p))
        }
        CoefficientDomain::ApproximateReal => unreachable!("validated earlier"),
    }
}
