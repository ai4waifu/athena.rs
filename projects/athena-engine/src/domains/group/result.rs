//! 群论域分派：置换 presentation、BSGS、子群/同态/商。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::domains::algebra::GroupTable;

use super::{
    canonical::{apply_group_homomorphism, inverse_group_element, multiply_group_elements, project_quotient_element},
    request::GroupRequest,
    value::GroupDomainValue,
};

/// 群论域结果。
#[derive(Debug, PartialEq)]
pub enum GroupResult {
    /// 精确结果。
    Exact {
        /// 值。
        value: GroupDomainValue,
    },
    /// 未求值。
    Unevaluated {
        /// 原因。
        reason: Diagnostic,
    },
}

/// 执行群论请求（无 Session 上下文）。
pub fn execute_group(request: GroupRequest) -> GroupResult {
    let op = operation_name(&request);
    GroupResult::Unevaluated {
        reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "group").detail("operation", op),
    }
}

/// 经 [`GroupTable`] 执行群论请求。
pub fn execute_group_with_table(request: GroupRequest, table: &GroupTable) -> GroupResult {
    match request {
        GroupRequest::Order { group } => match table.order(group) {
            Ok(order) => GroupResult::Exact { value: GroupDomainValue::Integer(order) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::Multiply { lhs, rhs } => match multiply_group_elements(table, &lhs, &rhs) {
            Ok(value) => GroupResult::Exact { value: GroupDomainValue::Element(value) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::Inverse { element } => match inverse_group_element(table, &element) {
            Ok(value) => GroupResult::Exact { value: GroupDomainValue::Element(value) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::IsAbelian { group } => match is_abelian(table, group) {
            Ok(v) => GroupResult::Exact { value: GroupDomainValue::Boolean(v) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::IsNormalSubgroup { subgroup } => match table.is_normal_subgroup(subgroup) {
            Ok(v) => GroupResult::Exact { value: GroupDomainValue::Boolean(v) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::ApplyHomomorphism { map, element } => match apply_group_homomorphism(table, map, &element) {
            Ok(value) => GroupResult::Exact { value: GroupDomainValue::Element(value) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::ProjectQuotient { subgroup, element } => match project_quotient_element(table, subgroup, &element) {
            Ok(value) => GroupResult::Exact { value: GroupDomainValue::Element(value) },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::PermutationGroup { .. }
        | GroupRequest::Cyclic { .. }
        | GroupRequest::SubgroupFromGenerators { .. }
        | GroupRequest::QuotientGroup { .. }
        | GroupRequest::HomomorphismFromGeneratorImages { .. } => GroupResult::Unevaluated {
            reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "group")
                .detail("operation", "register_requires_mutable_table"),
        },
    }
}

/// 可变 [`GroupTable`] 上执行（可 intern 新群）。
pub fn execute_group_with_table_mut(request: GroupRequest, table: &mut GroupTable) -> GroupResult {
    match request {
        GroupRequest::PermutationGroup { degree, generators } => match table.permutation_group(degree, &generators) {
            Ok(group) => match table.group_record(group) {
                Ok(g) => GroupResult::Exact { value: GroupDomainValue::Group(g) },
                Err(reason) => GroupResult::Unevaluated { reason },
            },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::SubgroupFromGenerators { parent, generators } => {
            match table.subgroup_from_generators(parent, &generators) {
                Ok(subgroup) => match table.subgroup_record(subgroup) {
                    Ok(s) => GroupResult::Exact { value: GroupDomainValue::Subgroup(s) },
                    Err(reason) => GroupResult::Unevaluated { reason },
                },
                Err(reason) => GroupResult::Unevaluated { reason },
            }
        }
        GroupRequest::QuotientGroup { subgroup } => match table.quotient_group(subgroup) {
            Ok(group) => match table.group_record(group) {
                Ok(g) => GroupResult::Exact { value: GroupDomainValue::Group(g) },
                Err(reason) => GroupResult::Unevaluated { reason },
            },
            Err(reason) => GroupResult::Unevaluated { reason },
        },
        GroupRequest::HomomorphismFromGeneratorImages { source, target, generator_images } => {
            match table.homomorphism_from_generator_images(source, target, &generator_images) {
                Ok(map) => GroupResult::Exact { value: GroupDomainValue::AlgebraMap(map) },
                Err(reason) => GroupResult::Unevaluated { reason },
            }
        }
        other => execute_group_with_table(other, table),
    }
}

fn is_abelian(table: &GroupTable, group: athena_types::GroupId) -> athena_types::Result<bool> {
    let spec = table.permutation_spec(group).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "group").detail("operation", "is_abelian")
    })?;
    let gens = &spec.generators;
    for (i, a) in gens.iter().enumerate() {
        for b in &gens[i + 1..] {
            let ab = a.compose(b).map_err(|_| Diagnostic::new(DiagnosticCode::PermutationInvalid))?;
            let ba = b.compose(a).map_err(|_| Diagnostic::new(DiagnosticCode::PermutationInvalid))?;
            if ab != ba {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn operation_name(request: &GroupRequest) -> &'static str {
    match request {
        GroupRequest::Cyclic { .. } => "cyclic",
        GroupRequest::PermutationGroup { .. } => "permutation_group",
        GroupRequest::Order { .. } => "order",
        GroupRequest::Multiply { .. } => "multiply",
        GroupRequest::Inverse { .. } => "inverse",
        GroupRequest::IsAbelian { .. } => "is_abelian",
        GroupRequest::SubgroupFromGenerators { .. } => "subgroup_from_generators",
        GroupRequest::IsNormalSubgroup { .. } => "is_normal_subgroup",
        GroupRequest::QuotientGroup { .. } => "quotient_group",
        GroupRequest::HomomorphismFromGeneratorImages { .. } => "homomorphism_from_generator_images",
        GroupRequest::ApplyHomomorphism { .. } => "apply_homomorphism",
        GroupRequest::ProjectQuotient { .. } => "project_quotient",
    }
}
