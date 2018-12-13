//! 伽罗瓦请求分派（经 [`FieldTable`] + [`GroupTable`]）。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::algebra::{FieldTable, GroupTable};

use super::{request::GaloisRequest, result::GaloisResult, value::GaloisDomainValue};

/// 经域表与群表执行伽罗瓦请求。
pub fn execute_galois_with_tables(request: GaloisRequest, fields: &mut FieldTable, groups: &mut GroupTable) -> GaloisResult {
    match request {
        GaloisRequest::IsExtensionSeparable { extension } => match crate::algebra::is_extension_separable(fields, extension) {
            Ok(v) => GaloisResult::Exact { value: GaloisDomainValue::Boolean(v) },
            Err(reason) => GaloisResult::Unevaluated { reason },
        },
        GaloisRequest::IsExtensionNormal { extension } => match crate::algebra::is_extension_normal(fields, extension) {
            Ok(v) => GaloisResult::Exact { value: GaloisDomainValue::Boolean(v) },
            Err(reason) => GaloisResult::Unevaluated { reason },
        },
        GaloisRequest::IsGalois { extension } => match crate::algebra::is_galois_extension(fields, extension) {
            Ok(v) => GaloisResult::Exact { value: GaloisDomainValue::Boolean(v) },
            Err(reason) => GaloisResult::Unevaluated { reason },
        },
        GaloisRequest::GaloisGroupOfExtension { extension } => {
            match crate::algebra::galois_group_of_extension(fields, groups, extension) {
                Ok(group) => GaloisResult::Exact { value: GaloisDomainValue::GaloisGroup(group) },
                Err(reason) => GaloisResult::Unevaluated { reason },
            }
        }
        GaloisRequest::IsPolynomialSeparable { .. }
        | GaloisRequest::GaloisGroupOfPolynomial { .. }
        | GaloisRequest::FixedField { .. } => GaloisResult::Unevaluated {
            reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "galois")
                .detail("operation", operation_name(&request)),
        },
    }
}

fn operation_name(request: &GaloisRequest) -> &'static str {
    match request {
        GaloisRequest::IsPolynomialSeparable { .. } => "is_polynomial_separable",
        GaloisRequest::IsExtensionNormal { .. } => "is_extension_normal",
        GaloisRequest::IsExtensionSeparable { .. } => "is_extension_separable",
        GaloisRequest::IsGalois { .. } => "is_galois",
        GaloisRequest::GaloisGroupOfPolynomial { .. } => "galois_group_of_polynomial",
        GaloisRequest::GaloisGroupOfExtension { .. } => "galois_group_of_extension",
        GaloisRequest::FixedField { .. } => "fixed_field",
    }
}
