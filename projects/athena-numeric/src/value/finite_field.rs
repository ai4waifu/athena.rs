//! 有限域元素骨架。

use athena_types::{Diagnostic, DiagnosticCode, FieldId, Result};

use crate::value::integer::Integer;

/// 有限域中的元素（canonical 系数 payload）。
///
/// 模多项式、基与约化计划由 engine `FieldPresentation` / `FieldTable` 持有，
/// 不得在本层重复或引用 IR。[`FieldId`] 是 Session-local 句柄，不是跨进程域身份。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteFieldValue {
    /// 域。
    pub field: FieldId,
    /// 约化后的基坐标（至少一项；零元编码为 `[0]`）。
    pub coefficients: Vec<Integer>,
}

impl FiniteFieldValue {
    /// 校验并构造。
    pub fn try_new(field: FieldId, coefficients: Vec<Integer>) -> Result<Self> {
        let v = Self { field, coefficients };
        v.validate()?;
        Ok(v)
    }

    /// 指定域中的零元。
    pub fn zero(field: FieldId) -> Self {
        Self { field, coefficients: vec![Integer::zero()] }
    }

    /// 不变量校验。
    pub fn validate(&self) -> Result<()> {
        if self.coefficients.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "finite_field_empty_coefficients"));
        }
        Ok(())
    }
}
