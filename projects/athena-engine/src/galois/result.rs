//! 伽罗瓦域分派（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{request::GaloisRequest, value::GaloisDomainValue};

/// 伽罗瓦结果。
#[derive(Debug, Clone, PartialEq)]
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

/// 执行伽罗瓦请求。
pub fn execute_galois(request: GaloisRequest) -> GaloisResult {
    let op = match &request {
        GaloisRequest::IsSeparable { .. } => "is_separable",
        GaloisRequest::IsNormal { .. } => "is_normal",
        GaloisRequest::GaloisGroup { .. } => "galois_group",
        GaloisRequest::FixedField { .. } => "fixed_field",
    };
    GaloisResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "galois").detail("operation", op),
    }
}
