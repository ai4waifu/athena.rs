//! 置换群注册表与 BSGS 缓存（Living `18` Phase 6）。

use std::collections::HashMap;

use athena_numeric::Integer;
use athena_types::{Diagnostic, DiagnosticCode, GroupId, PresentationId, Result};

use crate::group::{Group, GroupDescriptor, Permutation};

use super::{
    bsgs::BsgsChain,
    permutation::{RawPerm, validate_images},
    presentation::{GroupPresentation, GroupPresentationKind},
    property::{PropertyState, PropertyWitness},
};

/// 置换群 intern 规格。
#[derive(Debug, Clone)]
pub struct PermutationGroupSpec {
    /// 作用度数。
    pub degree: u32,
    /// 输入生成元（像列表）。
    pub generators: Vec<RawPerm>,
    /// Schreier–Sims BSGS 链。
    pub bsgs: BsgsChain,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct GroupInternKey {
    degree: u32,
    generators: Vec<Vec<u32>>,
}

/// Session 级群与 presentation 注册表。
#[derive(Debug, Default)]
pub struct GroupTable {
    next_group_id: u32,
    next_presentation_id: u32,
    presentations: HashMap<PresentationId, GroupPresentation>,
    group_to_presentation: HashMap<GroupId, PresentationId>,
    by_key: HashMap<GroupInternKey, GroupId>,
    permutation_groups: HashMap<GroupId, PermutationGroupSpec>,
}

impl GroupTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册置换群（生成元 + `Permutation` presentation + BSGS 链）。
    pub fn permutation_group(&mut self, degree: u32, generators: &[Permutation]) -> Result<GroupId> {
        let raw: Result<Vec<RawPerm>> = generators.iter().map(|g| RawPerm::new(g.images.clone(), degree)).collect();
        let raw = raw?;
        let key = GroupInternKey {
            degree,
            generators: raw.iter().map(|p| p.images().to_vec()).collect(),
        };
        if let Some(&id) = self.by_key.get(&key) {
            return Ok(id);
        }
        let bsgs = BsgsChain::from_generators(&raw, degree);
        let group = GroupId(self.next_group_id);
        self.next_group_id = self.next_group_id.wrapping_add(1);
        let presentation_id = PresentationId(self.next_presentation_id);
        self.next_presentation_id = self.next_presentation_id.wrapping_add(1);
        let presentation = GroupPresentation {
            id: presentation_id,
            group,
            kind: GroupPresentationKind::Permutation { degree },
        };
        self.by_key.insert(key, group);
        self.group_to_presentation.insert(group, presentation_id);
        self.presentations.insert(presentation_id, presentation);
        self.permutation_groups.insert(group, PermutationGroupSpec { degree, generators: raw, bsgs });
        Ok(group)
    }

    /// 置换群规格（若已注册）。
    pub fn permutation_spec(&self, group: GroupId) -> Option<&PermutationGroupSpec> {
        self.permutation_groups.get(&group)
    }

    /// 群 presentation。
    pub fn presentation(&self, group: GroupId) -> Option<&GroupPresentation> {
        self.group_to_presentation.get(&group).and_then(|id| self.presentations.get(id))
    }

    /// presentation id。
    pub fn presentation_id(&self, group: GroupId) -> Result<PresentationId> {
        self.group_to_presentation.get(&group).copied().ok_or_else(|| unknown_group(group))
    }

    /// 群阶（置换群经 BSGS；其他未支持）。
    pub fn order(&self, group: GroupId) -> Result<Integer> {
        let spec = self.permutation_spec(group).ok_or_else(|| unknown_group(group))?;
        Ok(spec.bsgs.order.clone())
    }

    /// 组装群对象。
    pub fn group_record(&self, group: GroupId) -> Result<Group> {
        let spec = self.permutation_spec(group).ok_or_else(|| unknown_group(group))?;
        Ok(Group {
            id: group,
            descriptor: GroupDescriptor::Abstract {
                order: PropertyState::Proven {
                    value: spec.bsgs.order.clone(),
                    witness: PropertyWitness::placeholder("bsgs_order"),
                },
                properties: Default::default(),
            },
            presentation: self.presentation_id(group)?,
            order: Some(spec.bsgs.order.clone()),
        })
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
