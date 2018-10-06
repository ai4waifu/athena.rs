//! 多项式环描述符与系数域合同。

use athena_numeric::{Integer, Modulus};
use athena_types::{Diagnostic, DiagnosticCode, FieldId, RingId, SymbolId};

use super::order::MonomialOrder;

/// 精确 / 近似系数域（环身份的一部分）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CoefficientDomain {
    /// ℤ（特征 0）。
    Integer,
    /// ℚ（特征 0）。
    Rational,
    /// 素域 𝔽_p（`p` 须为正；素性证明 F3 前为声明）。
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
    },
    /// 机器 / 近似实数 — 不得进入精确多项式环。
    ApproximateReal,
}

/// 环特征。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RingCharacteristic {
    /// 特征 0（ℤ、ℚ 等）。
    Zero,
    /// 特征 `p`（素域）。
    Prime(Integer),
}

/// 多项式环完整描述符（`RingId` 的内容）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingDescriptor {
    /// 稳定环 id。
    pub id: RingId,
    /// 系数域。
    pub coefficients: CoefficientDomain,
    /// 有序、无重复的变量身份。
    pub variables: Vec<SymbolId>,
    /// 单项式序（环身份，非算法选项）。
    pub order: MonomialOrder,
    /// 环特征（由系数域推导）。
    pub characteristic: RingCharacteristic,
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
        let characteristic = characteristic_of(&coefficients)?;
        Ok((coefficients, variables, order, characteristic))
    }

    /// 构造带 id 的描述符（intern 后使用）。
    pub(crate) fn with_id(
        id: RingId,
        coefficients: CoefficientDomain,
        variables: Vec<SymbolId>,
        order: MonomialOrder,
        characteristic: RingCharacteristic,
    ) -> Self {
        Self { id, coefficients, variables, order, characteristic }
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

fn characteristic_of(coeff: &CoefficientDomain) -> Result<RingCharacteristic, Diagnostic> {
    match coeff {
        CoefficientDomain::Integer | CoefficientDomain::Rational | CoefficientDomain::ModularInteger { .. } => {
            Ok(RingCharacteristic::Zero)
        }
        CoefficientDomain::PrimeField { p } => {
            if p.is_zero() || p.is_negative() {
                return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                    .detail("domain", "polynomial")
                    .detail("operation", "prime_field_characteristic"));
            }
            Ok(RingCharacteristic::Prime(p.clone()))
        }
        CoefficientDomain::FiniteField { .. } => Ok(RingCharacteristic::Zero),
        CoefficientDomain::ApproximateReal => unreachable!("validated earlier"),
    }
}
