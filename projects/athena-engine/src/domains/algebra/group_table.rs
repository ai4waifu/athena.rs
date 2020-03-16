//! 置换群注册表与 BSGS 缓存。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{AlgebraMapId, Diagnostic, DiagnosticCode, GroupId, GroupPresentationId, Result, SubgroupId};

use crate::domains::group::{Group, GroupDescriptor, Permutation, Subgroup};

use super::{
    bsgs::BsgsChain,
    fingerprint::GroupFingerprint,
    map_table::MapTable,
    permutation::{RawPerm, validate_images},
    presentation::{GroupPresentation, GroupPresentationKind},
    subgroup::{coset_index, is_normal, quotient_generators, verify_homomorphism_and_cache},
};
use crate::runtime::values::numeric_clone::clone_integer;

/// 置换群 intern 规格。
#[derive(Debug)]
pub struct PermutationGroupSpec {
    /// 作用度数。
    pub degree: u32,
    /// 输入生成元（像列表）。
    pub generators: Vec<RawPerm>,
    /// Schreier–Sims BSGS 链。
    pub bsgs: BsgsChain,
}

#[derive(Debug, PartialEq, Eq, Hash)]
struct GroupInternKey {
    degree: u32,
    generators: Vec<Vec<u32>>,
}

#[derive(Debug, PartialEq, Eq)]
struct SubgroupRecord {
    id: SubgroupId,
    parent: GroupId,
    subgroup: GroupId,
    inclusion: AlgebraMapId,
}

/// Session 级群与 presentation 注册表。
#[derive(Debug, Default)]
pub struct GroupTable {
    next_group_id: u32,
    next_presentation_id: u32,
    next_subgroup_id: u32,
    presentations: HashMap<GroupPresentationId, GroupPresentation>,
    group_to_presentation: HashMap<GroupId, GroupPresentationId>,
    by_key: HashMap<GroupInternKey, GroupId>,
    permutation_groups: HashMap<GroupId, PermutationGroupSpec>,
    map_table: MapTable,
    subgroups: HashMap<SubgroupId, SubgroupRecord>,
}

impl GroupTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 映射表（子群包含、同态、商投影）。
    pub fn map_table(&self) -> &MapTable {
        &self.map_table
    }

    /// 注册置换群（生成元 + `Permutation` presentation + BSGS 链）。
    pub fn permutation_group(&mut self, degree: u32, generators: &[Permutation]) -> Result<GroupId> {
        let raw: Result<Vec<RawPerm>> = generators.iter().map(|g| RawPerm::new(g.images.clone(), degree)).collect();
        let raw = raw?;
        let key = GroupInternKey { degree, generators: raw.iter().map(|p| p.images().to_vec()).collect() };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let bsgs = BsgsChain::from_generators(&raw, degree);
        let group = GroupId(self.next_group_id);
        self.next_group_id = self.next_group_id.wrapping_add(1);
        let presentation_id = GroupPresentationId(self.next_presentation_id);
        self.next_presentation_id = self.next_presentation_id.wrapping_add(1);
        let presentation = GroupPresentation { id: presentation_id, group, kind: GroupPresentationKind::Permutation { degree } };
        self.by_key.insert(key, group);
        self.group_to_presentation.insert(group, presentation_id);
        self.presentations.insert(presentation_id, presentation);
        self.permutation_groups.insert(group, PermutationGroupSpec { degree, generators: raw, bsgs });
        Ok(group)
    }

    /// 由父群生成元构造子群 H ≤ G。
    pub fn subgroup_from_generators(&mut self, parent: GroupId, generators: &[Permutation]) -> Result<SubgroupId> {
        let parent_spec = self.permutation_spec(parent).ok_or_else(|| unknown_group(parent))?;
        let raw: Result<Vec<RawPerm>> = generators.iter().map(|g| RawPerm::new(g.images.clone(), parent_spec.degree)).collect();
        let raw = raw?;
        for g in &raw {
            if !parent_spec.bsgs.contains(g) {
                return Err(subgroup_invalid("generator_not_in_parent"));
            }
        }
        let subgroup_group = self.permutation_group(parent_spec.degree, generators)?;
        let subgroup_id = SubgroupId(self.next_subgroup_id);
        self.next_subgroup_id = self.next_subgroup_id.wrapping_add(1);
        let sub_presentation = self.presentation_id(subgroup_group)?;
        let parent_presentation = self.presentation_id(parent)?;
        let inclusion = self.map_table.register_subgroup_inclusion(subgroup_id, subgroup_group, parent, sub_presentation, parent_presentation);
        self.subgroups.insert(subgroup_id, SubgroupRecord { id: subgroup_id, parent, subgroup: subgroup_group, inclusion });
        Ok(subgroup_id)
    }

    /// 子群记录。
    pub fn subgroup_record(&self, subgroup: SubgroupId) -> Result<Subgroup> {
        let r = self.subgroups.get(&subgroup).ok_or_else(|| unknown_subgroup(subgroup))?;
        Ok(Subgroup { id: r.id, parent: r.parent, group: r.subgroup, inclusion: r.inclusion })
    }

    /// 子群是否正规于父群。
    pub fn is_normal_subgroup(&self, subgroup: SubgroupId) -> Result<bool> {
        let r = self.subgroups.get(&subgroup).ok_or_else(|| unknown_subgroup(subgroup))?;
        let parent_spec = self.permutation_spec(r.parent).ok_or_else(|| unknown_group(r.parent))?;
        let sub_spec = self.permutation_spec(r.subgroup).ok_or_else(|| unknown_group(r.subgroup))?;
        Ok(is_normal(&parent_spec.bsgs, &parent_spec.generators, &sub_spec.bsgs))
    }

    /// 构造商群 G/N（N 须正规）。
    pub fn quotient_group(&mut self, subgroup: SubgroupId) -> Result<GroupId> {
        if let Some(q) = self.map_table.quotient_group(subgroup) {
            return Ok(q);
        }
        if !self.is_normal_subgroup(subgroup)? {
            return Err(Diagnostic::new(DiagnosticCode::GroupNotNormal).detail("domain", "group").detail("operation", "quotient"));
        }
        let (parent, subgroup_gid) = {
            let r = self.subgroups.get(&subgroup).ok_or_else(|| unknown_subgroup(subgroup))?;
            (r.parent, r.subgroup)
        };
        let parent_spec = self.permutation_spec(parent).ok_or_else(|| unknown_group(parent))?;
        let sub_spec = self.permutation_spec(subgroup_gid).ok_or_else(|| unknown_group(subgroup_gid))?;
        let (gens, degree) = quotient_generators(&parent_spec.bsgs, &parent_spec.generators, &sub_spec.bsgs)?;
        let perm_gens: Vec<Permutation> = gens.iter().map(|g| Permutation { images: g.images().to_vec() }).collect();
        let quotient = self.permutation_group(degree, &perm_gens)?;
        let parent_presentation = self.presentation_id(parent)?;
        let quotient_presentation = self.presentation_id(quotient)?;
        self.map_table.register_quotient_projection(subgroup, parent, quotient, parent_presentation, quotient_presentation);
        Ok(quotient)
    }

    /// 由源群生成元像注册同态（`GeneratorRelations` 验证）。
    pub fn homomorphism_from_generator_images(
        &mut self,
        source: GroupId,
        target: GroupId,
        generator_images: &[Permutation],
    ) -> Result<AlgebraMapId> {
        let source_spec = self.permutation_spec(source).ok_or_else(|| unknown_group(source))?;
        let target_spec = self.permutation_spec(target).ok_or_else(|| unknown_group(target))?;
        let images: Result<Vec<RawPerm>> = generator_images.iter().map(|p| RawPerm::new(p.images.clone(), target_spec.degree)).collect();
        let images = images?;
        let cache = verify_homomorphism_and_cache(&source_spec.bsgs, &source_spec.generators, &target_spec.bsgs, &images)?;
        let source_presentation = self.presentation_id(source)?;
        let target_presentation = self.presentation_id(target)?;
        Ok(self.map_table.register_group_homomorphism(source, target, source_presentation, target_presentation, cache))
    }

    /// 经已验证同态映射元素像。
    pub fn apply_homomorphism(&self, map: AlgebraMapId, element_images: &[u32]) -> Result<RawPerm> {
        self.map_table
            .homomorphism_image(map, element_images)
            .map(RawPerm::owning_copy)
            .ok_or_else(|| hom_invalid("unknown_preimage"))
    }

    /// 经商投影将父群元素映到商群置换元素。
    pub fn project_quotient(&self, subgroup: SubgroupId, parent_element: &RawPerm) -> Result<RawPerm> {
        let r = self.subgroups.get(&subgroup).ok_or_else(|| unknown_subgroup(subgroup))?;
        let parent_spec = self.permutation_spec(r.parent).ok_or_else(|| unknown_group(r.parent))?;
        let sub_spec = self.permutation_spec(r.subgroup).ok_or_else(|| unknown_group(r.subgroup))?;
        if !parent_spec.bsgs.contains(parent_element) {
            return Err(subgroup_invalid("element_not_in_parent"));
        }
        let reps = super::subgroup::coset_representatives(&parent_spec.bsgs, &sub_spec.bsgs);
        let index = reps.len() as u32;
        let mut images = vec![0u32; index as usize];
        for (i, rep) in reps.iter().enumerate() {
            let product = rep.compose(parent_element)?;
            let j = coset_index(&reps, &sub_spec.bsgs, &product).ok_or_else(|| hom_invalid("coset_not_found"))?;
            images[i] = j as u32;
        }
        RawPerm::new(images, index)
    }

    /// 置换群规格（若已注册）。
    pub fn permutation_spec(&self, group: GroupId) -> Option<&PermutationGroupSpec> {
        self.permutation_groups.get(&group)
    }

    /// 群 presentation。
    pub fn presentation(&self, group: GroupId) -> Option<&GroupPresentation> {
        self.group_to_presentation.get(&group).and_then(|id| self.presentations.get(id))
    }

    /// presentation 标识。
    pub fn presentation_id(&self, group: GroupId) -> Result<GroupPresentationId> {
        self.group_to_presentation.get(&group).copied().ok_or_else(|| unknown_group(group))
    }

    /// 群阶（置换群经 BSGS；其他未支持）。
    pub fn order(&self, group: GroupId) -> Result<Integer> {
        let spec = self.permutation_spec(group).ok_or_else(|| unknown_group(group))?;
        Ok(clone_integer(&spec.bsgs.order))
    }

    /// 组装群对象。
    pub fn group_record(&self, group: GroupId) -> Result<Group> {
        let spec = self.permutation_spec(group).ok_or_else(|| unknown_group(group))?;
        Ok(Group {
            id: group,
            descriptor: GroupDescriptor::Permutation { degree: spec.degree },
            presentation: self.presentation_id(group)?,
            order: Some(clone_integer(&spec.bsgs.order)),
        })
    }

    /// 群内容指纹（跨 Session 可比较；不含 [`GroupId`]）。
    pub fn group_fingerprint(&self, group: GroupId) -> Option<GroupFingerprint> {
        let spec = self.permutation_spec(group)?;
        let gens: Vec<Vec<u32>> = spec.generators.iter().map(|g| g.images().to_vec()).collect();
        Some(GroupFingerprint::from_permutation_generators(spec.degree, &gens))
    }

    /// 要求群已绑定可计算 presentation（抽象仅知阶的 descriptor 拒入计算 API）。
    pub fn ensure_computable(&self, group: GroupId) -> Result<()> {
        if self.permutation_spec(group).is_some() {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "group")
                .detail("operation", "abstract_descriptor_not_computable")
                .detail("group_id", group.0.to_string()))
        }
    }

    /// 校验置换属于已注册群。
    pub fn validate_permutation(&self, group: GroupId, images: &[u32]) -> Result<()> {
        let spec = self.permutation_spec(group).ok_or_else(|| unknown_group(group))?;
        validate_images(images, spec.degree)?;
        Ok(())
    }
}

fn unknown_group(group: GroupId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "group")
        .detail("operation", "unknown_group")
        .detail("group_id", group.0.to_string())
}

fn unknown_subgroup(subgroup: SubgroupId) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "group")
        .detail("operation", "unknown_subgroup")
        .detail("subgroup_id", subgroup.0.to_string())
}

fn subgroup_invalid(operation: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::GroupElementInvalid).detail("domain", "group").detail("operation", operation)
}

fn hom_invalid(operation: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::GroupElementInvalid).detail("domain", "group").detail("operation", operation)
}
