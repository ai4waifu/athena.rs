//! 伽罗瓦域分派（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{request::GaloisRequest, value::GaloisDomainValue};

/// 伽罗瓦结果。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub enum GaloisResult {
    /// 精确结果。
    Exact {
        /// 值。
        value: GaloisDomainValue,
    },
    /// 未求值。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

impl GaloisResult {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Exact { value } => Self::Exact { value: value.owning_copy() },
            Self::Unevaluated { reason } => Self::Unevaluated { reason: reason.clone() },
        }
    }
}

/// 执行伽罗瓦请求。
pub fn execute_galois(request: GaloisRequest) -> GaloisResult {
    let op = match &request {
        GaloisRequest::IsPolynomialSeparable { .. } => "is_polynomial_separable",
        GaloisRequest::IsExtensionNormal { .. } => "is_extension_normal",
        GaloisRequest::IsExtensionSeparable { .. } => "is_extension_separable",
        GaloisRequest::IsGalois { .. } => "is_galois",
        GaloisRequest::GaloisGroupOfPolynomial { .. } => "galois_group_of_polynomial",
        GaloisRequest::GaloisGroupOfExtension { .. } => "galois_group_of_extension",
        GaloisRequest::FixedField { .. } => "fixed_field",
    };
    GaloisResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "galois").detail("operation", op),
    }
}
