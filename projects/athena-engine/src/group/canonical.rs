//! 置换群元素 canonical 化与运算。

use athena_types::{AlgebraMapId, Diagnostic, DiagnosticCode, GroupElementId, GroupId, Result, SubgroupId};

use crate::algebra::{GroupTable, RawPerm};

use super::types::{GroupElement, GroupElementRepr, Permutation};

/// 构造 canonical 置换元素。
pub fn canonical_permutation(
    table: &GroupTable,
    group: GroupId,
    images: Vec<u32>,
    element_id: GroupElementId,
) -> Result<GroupElement> {
    table.ensure_computable(group)?;
    table.validate_permutation(group, &images)?;
    let presentation = table.presentation_id(group)?;
    Ok(GroupElement { id: element_id, group, presentation, repr: GroupElementRepr::Permutation(Permutation { images }) })
}

/// 置换群元素乘法（合成 `p(q(i))`）。
pub fn multiply_group_elements(table: &GroupTable, lhs: &GroupElement, rhs: &GroupElement) -> Result<GroupElement> {
    table.ensure_computable(lhs.group)?;
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
    table.ensure_computable(element.group)?;
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
    table.ensure_computable(group)?;
    let spec = table.permutation_spec(group).ok_or_else(|| group_element_invalid("not_permutation_group"))?;
    let raw = match &element.repr {
        GroupElementRepr::Permutation(p) => raw_perm(p, table, group)?,
        _ => return Err(group_element_invalid("membership_not_permutation")),
    };
    Ok(spec.bsgs.contains(&raw))
}

/// 经已验证同态映射元素。
pub fn apply_group_homomorphism(table: &GroupTable, map: AlgebraMapId, element: &GroupElement) -> Result<GroupElement> {
    let algebra_map = table.map_table().get(map).ok_or_else(|| group_element_invalid("unknown_homomorphism"))?;
    algebra_map.require_proven()?;
    let source = table.map_table().homomorphism_source(map).ok_or_else(|| group_element_invalid("unknown_homomorphism"))?;
    table.ensure_computable(source)?;
    if element.group != source {
        return Err(group_mismatch());
    }
    let raw = match &element.repr {
        GroupElementRepr::Permutation(p) => raw_perm(p, table, source)?,
        _ => return Err(group_element_invalid("homomorphism_not_permutation")),
    };
    let image = table.apply_homomorphism(map, raw.images())?;
    let target = table.map_table().homomorphism_target(map).ok_or_else(|| group_element_invalid("unknown_homomorphism"))?;
    table.ensure_computable(target)?;
    canonical_permutation(table, target, image.images().to_vec(), GroupElementId(0))
}

/// 经商投影映射父群元素到商群。
pub fn project_quotient_element(table: &GroupTable, subgroup: SubgroupId, element: &GroupElement) -> Result<GroupElement> {
    let record = table.subgroup_record(subgroup)?;
    table.ensure_computable(record.parent)?;
    if let Some(proj) = table.map_table().quotient_projection_map(subgroup) {
        proj.require_proven()?;
    }
    else {
        return Err(group_element_invalid("quotient_not_registered"));
    }
    if element.group != record.parent {
        return Err(group_mismatch());
    }
    let raw = match &element.repr {
        GroupElementRepr::Permutation(p) => raw_perm(p, table, record.parent)?,
        _ => return Err(group_element_invalid("quotient_not_permutation")),
    };
    let image = table.project_quotient(subgroup, &raw)?;
    let quotient =
        table.map_table().quotient_group(subgroup).ok_or_else(|| group_element_invalid("quotient_not_registered"))?;
    table.ensure_computable(quotient)?;
    canonical_permutation(table, quotient, image.images().to_vec(), GroupElementId(0))
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
