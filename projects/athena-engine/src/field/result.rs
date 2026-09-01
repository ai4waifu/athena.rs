//! 域论域分派（骨架）。

use athena_types::{Diagnostic, DiagnosticCode};

use super::{request::FieldRequest, value::FieldDomainValue};

/// 域论结果。
#[derive(Debug, Clone, PartialEq)]
pub enum FieldResult {
    /// 精确结果。
    Exact {
        /// 值。
        value: FieldDomainValue,
    },
    /// 未求值。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

/// 执行域论请求。
pub fn execute_field(request: FieldRequest) -> FieldResult {
    let op = match &request {
        FieldRequest::PrimeField { .. } => "prime_field",
        FieldRequest::Rationals => "rationals",
        FieldRequest::Add { .. } => "add",
        FieldRequest::Mul { .. } => "mul",
        FieldRequest::Inverse { .. } => "inverse",
        FieldRequest::Lookup { .. } => "lookup",
    };
    FieldResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "field").detail("operation", op),
    }
}
