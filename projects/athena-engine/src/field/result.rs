//! 域论域分派：ℚ / 𝔽_p / 𝔽_{p^n} canonical 运算。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::algebra::FieldTable;

use super::{
    canonical::{add_field_elements, inv_field_element, mul_field_elements},
    request::FieldRequest,
    value::FieldDomainValue,
};

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

/// 执行域论请求（无 Session 上下文；仍返回 Unevaluated）。
pub fn execute_field(request: FieldRequest) -> FieldResult {
    let op = operation_name(&request);
    FieldResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "field").detail("operation", op),
    }
}

/// 经 [`FieldTable`] 执行域论请求（ℚ / 𝔽_p / 𝔽_{p^n}）。
pub fn execute_field_with_table(request: FieldRequest, table: &FieldTable) -> FieldResult {
    match request {
        FieldRequest::Rationals => {
            if let Some(q) = table.rationals_field() {
                match table.field_record(q) {
                    Ok(f) => FieldResult::Exact { value: FieldDomainValue::Field(f) },
                    Err(reason) => FieldResult::Unevaluated { reason },
                }
            }
            else {
                FieldResult::Unevaluated {
                    reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("domain", "field")
                        .detail("operation", "rationals_requires_mutable_table"),
                }
            }
        }
        FieldRequest::PrimeField { characteristic: _ } => FieldResult::Unevaluated {
            reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "field")
                .detail("operation", "prime_field_requires_mutable_table"),
        },
        FieldRequest::Lookup { field } => match table.field_record(field) {
            Ok(f) => FieldResult::Exact { value: FieldDomainValue::Field(f) },
            Err(reason) => FieldResult::Unevaluated { reason },
        },
        FieldRequest::Add { lhs, rhs } => run_binary(table, add_field_elements, lhs, rhs),
        FieldRequest::Mul { lhs, rhs } => run_binary(table, mul_field_elements, lhs, rhs),
        FieldRequest::Inverse { element } => match inv_field_element(table, &element) {
            Ok(value) => FieldResult::Exact { value: FieldDomainValue::Element(value) },
            Err(reason) => FieldResult::Unevaluated { reason },
        },
    }
}

/// 可变 [`FieldTable`] 上执行（可 intern 新域）。
pub fn execute_field_with_table_mut(request: FieldRequest, table: &mut FieldTable) -> FieldResult {
    match request {
        FieldRequest::Rationals => {
            let q = table.rationals();
            FieldResult::Exact { value: FieldDomainValue::Field(table.field_record(q).unwrap()) }
        }
        FieldRequest::PrimeField { characteristic } => match table.prime_field(characteristic) {
            Ok(f) => FieldResult::Exact { value: FieldDomainValue::Field(table.field_record(f).unwrap()) },
            Err(reason) => FieldResult::Unevaluated { reason },
        },
        other => execute_field_with_table(other, table),
    }
}

fn run_binary<F>(table: &FieldTable, op: F, lhs: super::types::FieldElement, rhs: super::types::FieldElement) -> FieldResult
where
    F: FnOnce(
        &FieldTable,
        &super::types::FieldElement,
        &super::types::FieldElement,
    ) -> athena_types::Result<super::types::FieldElement>,
{
    match op(table, &lhs, &rhs) {
        Ok(value) => FieldResult::Exact { value: FieldDomainValue::Element(value) },
        Err(reason) => FieldResult::Unevaluated { reason },
    }
}

fn operation_name(request: &FieldRequest) -> &'static str {
    match request {
        FieldRequest::PrimeField { .. } => "prime_field",
        FieldRequest::Rationals => "rationals",
        FieldRequest::Add { .. } => "add",
        FieldRequest::Mul { .. } => "mul",
        FieldRequest::Inverse { .. } => "inverse",
        FieldRequest::Lookup { .. } => "lookup",
    }
}
