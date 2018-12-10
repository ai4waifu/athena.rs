//! 置换群元素 canonical 化与运算（Living `18` Phase 6）。

use athena_types::{Diagnostic, DiagnosticCode, GroupElementId, GroupId, Result};

use crate::algebra::{GroupTable, RawPerm};

use super::types::{GroupElement, GroupElementRepr, Permutation};

/// 构造 canonical 置换元素。
pub fn canonical_permutation(
    table: &GroupTable,
    group: GroupId,
    images: Vec<u32>,
    element_id: GroupElementId,
) -> Result<GroupElement> {
    table.validate_permutation(group, &images)?;
    let presentation = table.presentation_id(group)?;
    Ok(GroupElement {
        id: element_id,
        group,
        presentation,
        repr: GroupElementRepr::Permutation(Permutation { images }),
    })
}

/// 置换群元素乘法（合成 `p(q(i))`）。
pub fn multiply_group_elements(table: &GroupTable, lhs: &GroupElement, rhs: &GroupElement) -> Result<GroupElement> {
    ensure_same_group(lhs, rhs)?;
    match (&lhs.repr, &rhs.repr) {
        (GroupElementRepr::Permutation(a), GroupElementRepr::Permutation(b)) => {
            let pa = raw_perm(a, table, lhs.group)?;
            let pb = raw_perm(b, table, rhs.group)?;
            let product = pa.compose(&pb)?;
            canonical_permutation(table, lhs.group, product.images().to_vec(), GroupElementId(0))
        }
        _ => Err(group_element_invalid("multiply_repr_mismatch")),
    }
}

/// 置换逆元。
pub fn inverse_group_element(table: &GroupTable, element: &GroupElement) -> Result<GroupElement> {
    match &element.repr {
        GroupElementRepr::Permutation(p) => {
            let raw = raw_perm(p, table, element.group)?;
            let inv = raw.inverse();
            canonical_permutation(table, element.group, inv.images().to_vec(), element.id)
        }
        _ => Err(group_element_invalid("inverse_unsupported_repr")),
    }
}

/// 成员判定（经 BSGS sift）。
pub fn group_membership(table: &GroupTable, group: GroupId, element: &GroupElement) -> Result<bool> {
    let spec = table.permutation_spec(group).ok_or_else(|| group_element_invalid("not_permutation_group"))?;
    let raw = match &element.repr {
        GroupElementRepr::Permutation(p) => raw_perm(p, table, group)?,
        _ => return Err(group_element_invalid("membership_not_permutation")),
    };
    Ok(spec.bsgs.contains(&raw))
}

fn raw_perm(p: &Permutation, table: &GroupTable, group: GroupId) -> Result<RawPerm> {
    let spec = table.permutation_spec(group).ok_or_else(|| group_element_invalid("not_permutation_group"))?;
    RawPerm::new(p.images.clone(), spec.degree)
}

fn ensure_same_group(lhs: &GroupElement, rhs: &GroupElement) -> Result<()> {
    if lhs.group != rhs.group || lhs.presentation != rhs.presentation {
        return Err(group_mismatch());
    }
    Ok(())
}

fn group_mismatch() -> Diagnostic {
    Diagnostic::new(DiagnosticCode::GroupMismatch).detail("domain", "group")
}

fn group_element_invalid(operation: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::GroupElementInvalid).detail("domain", "group").detail("operation", operation)
}
