//! 有限域元素骨架。
//!
//! 元素表示与基础不变量在 numeric。域 presentation、素性/不可约证明与域算法在 engine。

use athena_types::{Diagnostic, DiagnosticCode, FieldId, FieldPresentationId, Result};

use crate::value::integer::Integer;

/// 有限域元素物理表示（Living `19`）。
///
/// `Coefficients` 仅 bootstrap / wire。扩域热路径终局为 packed residue（后续）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FiniteFieldRepr {
    /// Bootstrap / wire：约化基坐标（至少一项；零元为 `[0]`）。
    Coefficients(Vec<Integer>),
}

/// 有限域中的元素。
///
/// [`FieldId`] 为抽象域身份；[`FieldPresentationId`] 为具体素模 / 不可约 / 基。
/// 禁止公开字段 struct literal 绕过校验。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteFieldValue {
    field: FieldId,
    presentation: FieldPresentationId,
    repr: FiniteFieldRepr,
}

impl FiniteFieldValue {
    /// 由 bootstrap 系数向量构造。
    pub fn try_new(field: FieldId, presentation: FieldPresentationId, coefficients: Vec<Integer>) -> Result<Self> {
        Self::try_from_repr(field, presentation, FiniteFieldRepr::Coefficients(coefficients))
    }

    /// 由表示构造。
    pub fn try_from_repr(field: FieldId, presentation: FieldPresentationId, repr: FiniteFieldRepr) -> Result<Self> {
        let v = Self { field, presentation, repr };
        v.validate()?;
        Ok(v)
    }

    /// 指定域 / presentation 中的零元。
    pub fn zero(field: FieldId, presentation: FieldPresentationId) -> Self {
        Self { field, presentation, repr: FiniteFieldRepr::Coefficients(vec![Integer::zero()]) }
    }

    /// 抽象域身份。
    pub fn field(&self) -> FieldId {
        self.field
    }

    /// Presentation 身份（素模 / 不可约 / 基）。
    pub fn presentation(&self) -> FieldPresentationId {
        self.presentation
    }

    /// 表示。
    pub fn repr(&self) -> &FiniteFieldRepr {
        &self.repr
    }

    /// 基坐标（bootstrap 布局；非 `Coefficients` 时为空切片）。
    pub fn coefficients(&self) -> &[Integer] {
        match &self.repr {
            FiniteFieldRepr::Coefficients(c) => c,
        }
    }

    /// 不变量校验。
    pub fn validate(&self) -> Result<()> {
        match &self.repr {
            FiniteFieldRepr::Coefficients(c) if c.is_empty() => Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "finite_field_empty_coefficients")),
            FiniteFieldRepr::Coefficients(_) => Ok(()),
        }
    }
}
