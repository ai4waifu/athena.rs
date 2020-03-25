//! 代数数骨架。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{interval::Interval, polynomial_fingerprint::PolynomialFingerprint};

/// 代数数表示策略。
#[derive(Debug, PartialEq, Eq)]
pub enum AlgebraicRepresentation {
    /// 极小多项式 + 隔离区间。
    MinimalPolynomial {
        /// 稳定极小多项式指纹（非 Session 局部 IR）。
        polynomial: PolynomialFingerprint,
        /// 根下标（相对明确根排序与隔离证书）。
        root_index: u32,
    },
    /// 占位。
    Placeholder,
}

/// 代数数。
///
/// 不变量：
/// - [`AlgebraicRepresentation::Placeholder`] 要求指纹为 [`PolynomialFingerprint::PLACEHOLDER`]。
/// - [`AlgebraicRepresentation::MinimalPolynomial`] 的指纹必须与 [`Self::minimal_polynomial`] 一致。
/// - 隔离区间不得为空。
#[derive(Debug, PartialEq)]
pub struct AlgebraicNumber {
    /// 极小多项式指纹。
    pub minimal_polynomial: PolynomialFingerprint,
    /// 实隔离区间（非实代数数须用复隔离区域，后续）。
    pub isolating_interval: Interval,
    /// 表示。
    pub representation: AlgebraicRepresentation,
}

impl AlgebraicNumber {
    /// Owning 深复制。
    pub fn try_clone_in(&self, ctx: &crate::execution_budget::NumericContext) -> Result<Self> {
        let representation = match &self.representation {
            AlgebraicRepresentation::Placeholder => AlgebraicRepresentation::Placeholder,
            AlgebraicRepresentation::MinimalPolynomial { polynomial, root_index } => {
                AlgebraicRepresentation::MinimalPolynomial { polynomial: *polynomial, root_index: *root_index }
            }
        };
        Ok(Self { minimal_polynomial: self.minimal_polynomial, isolating_interval: self.isolating_interval.try_clone_in(ctx)?, representation })
    }

    /// 校验并构造。
    pub fn try_new(
        minimal_polynomial: PolynomialFingerprint,
        isolating_interval: Interval,
        representation: AlgebraicRepresentation,
    ) -> Result<Self> {
        let v = Self { minimal_polynomial, isolating_interval, representation };
        v.validate()?;
        Ok(v)
    }

    /// 占位代数数（测试 / 未求值路径）。
    pub fn placeholder(isolating_interval: Interval) -> Result<Self> {
        Self::try_new(PolynomialFingerprint::PLACEHOLDER, isolating_interval, AlgebraicRepresentation::Placeholder)
    }

    /// 不变量校验。
    pub fn validate(&self) -> Result<()> {
        if self.isolating_interval.is_empty() {
            return Err(invalid("algebraic_empty_interval"));
        }
        match &self.representation {
            AlgebraicRepresentation::Placeholder => {
                if self.minimal_polynomial != PolynomialFingerprint::PLACEHOLDER {
                    return Err(invalid("algebraic_placeholder_fingerprint"));
                }
            }
            AlgebraicRepresentation::MinimalPolynomial { polynomial, .. } => {
                if *polynomial != self.minimal_polynomial {
                    return Err(invalid("algebraic_fingerprint_mismatch"));
                }
            }
        }
        Ok(())
    }
}

fn invalid(operation: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "numeric").detail("operation", operation)
}
