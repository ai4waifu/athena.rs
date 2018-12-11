//! 代数映射注册表（canonical embedding、子群包含、同态、商投影）。

use std::collections::HashMap;

use athena_types::{AlgebraMapId, FieldId, GroupId, PresentationId, SubgroupId};

use super::{
    map::{
        AlgebraMap, AlgebraMapKind, FieldEmbedding, GroupHomomorphism, MapVerification, MapVerificationKind,
        QuotientProjection, SubgroupInclusion,
    },
    parent::AlgebraParentId,
    permutation::RawPerm,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldEmbeddingRecord {
    map: AlgebraMap,
    embedding: FieldEmbedding,
}

#[derive(Debug, Clone)]
struct GroupHomomorphismRecord {
    map: AlgebraMap,
    homomorphism: GroupHomomorphism,
    source: GroupId,
    target: GroupId,
    element_images: HashMap<Vec<u32>, RawPerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubgroupInclusionRecord {
    map: AlgebraMap,
    inclusion: SubgroupInclusion,
    parent: GroupId,
    subgroup_group: GroupId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QuotientProjectionRecord {
    map: AlgebraMap,
    projection: QuotientProjection,
    parent: GroupId,
    quotient: GroupId,
}

/// Session 级映射表。
#[derive(Debug, Default)]
pub struct MapTable {
    next_id: u32,
    maps: HashMap<AlgebraMapId, AlgebraMap>,
    embeddings: HashMap<(FieldId, FieldId), FieldEmbeddingRecord>,
    homomorphisms: HashMap<AlgebraMapId, GroupHomomorphismRecord>,
    subgroup_inclusions: HashMap<SubgroupId, SubgroupInclusionRecord>,
    quotient_projections: HashMap<SubgroupId, QuotientProjectionRecord>,
}

impl MapTable {
    /// 空表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 按 id 查映射。
    pub fn get(&self, id: AlgebraMapId) -> Option<&AlgebraMap> {
        self.maps.get(&id)
    }

    /// 查 canonical embedding（source → target）。
    pub fn canonical_embedding(&self, source: FieldId, target: FieldId) -> Option<AlgebraMapId> {
        self.embeddings.get(&(source, target)).map(|r| r.map.id)
    }

    /// 查域嵌入记录。
    pub fn field_embedding(&self, id: AlgebraMapId) -> Option<&FieldEmbedding> {
        self.embeddings.values().find(|r| r.map.id == id).map(|r| &r.embedding)
    }

    /// 群同态记录。
    pub(crate) fn group_homomorphism(&self, id: AlgebraMapId) -> Option<&GroupHomomorphismRecord> {
        self.homomorphisms.get(&id)
    }

    /// 子群包含记录。
    pub(crate) fn subgroup_inclusion(&self, subgroup: SubgroupId) -> Option<&SubgroupInclusionRecord> {
        self.subgroup_inclusions.get(&subgroup)
    }

    /// 商投影记录。
    pub(crate) fn quotient_projection(&self, subgroup: SubgroupId) -> Option<&QuotientProjectionRecord> {
        self.quotient_projections.get(&subgroup)
    }

    /// 同态下元素像（源置换像列表）。
    pub fn homomorphism_image(&self, map: AlgebraMapId, source_images: &[u32]) -> Option<&RawPerm> {
        self.homomorphisms.get(&map)?.element_images.get(source_images)
    }

    /// 同态源群。
    pub fn homomorphism_source(&self, map: AlgebraMapId) -> Option<GroupId> {
        self.homomorphisms.get(&map).map(|r| r.source)
    }

    /// 同态靶群。
    pub fn homomorphism_target(&self, map: AlgebraMapId) -> Option<GroupId> {
        self.homomorphisms.get(&map).map(|r| r.target)
    }

    /// 商投影的商群 id。
    pub fn quotient_group(&self, subgroup: SubgroupId) -> Option<GroupId> {
        self.quotient_projections.get(&subgroup).map(|r| r.quotient)
    }

    /// 商投影的父群 id。
    pub fn quotient_parent(&self, subgroup: SubgroupId) -> Option<GroupId> {
        self.quotient_projections.get(&subgroup).map(|r| r.parent)
    }

    /// 注册 ℚ → 𝔽_p canonical embedding（幂等）。
    pub fn register_canonical_q_to_fp(
        &mut self,
        source: FieldId,
        target: FieldId,
        source_presentation: PresentationId,
        target_presentation: PresentationId,
    ) -> AlgebraMapId {
        if let Some(id) = self.canonical_embedding(source, target) {
            return id;
        }
        let id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id,
            source: AlgebraParentId::Field(source),
            target: AlgebraParentId::Field(target),
            kind: AlgebraMapKind::FieldEmbedding,
            verification: MapVerification { kind: MapVerificationKind::DegreeCheck, verified: true },
        };
        let embedding = FieldEmbedding { map: id, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.embeddings.insert((source, target), FieldEmbeddingRecord { map, embedding });
        id
    }

    /// 注册子群包含 H ↪ G。
    pub fn register_subgroup_inclusion(
        &mut self,
        subgroup: SubgroupId,
        subgroup_group: GroupId,
        parent: GroupId,
        source_presentation: PresentationId,
        target_presentation: PresentationId,
    ) -> AlgebraMapId {
        if let Some(r) = self.subgroup_inclusions.get(&subgroup) {
            return r.map.id;
        }
        let id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id,
            source: AlgebraParentId::Group(subgroup_group),
            target: AlgebraParentId::Group(parent),
            kind: AlgebraMapKind::SubgroupInclusion,
            verification: MapVerification { kind: MapVerificationKind::DegreeCheck, verified: true },
        };
        let inclusion = SubgroupInclusion {
            map: id,
            subgroup,
            source_presentation,
            target_presentation,
        };
        self.maps.insert(id, map.clone());
        self.subgroup_inclusions.insert(
            subgroup,
            SubgroupInclusionRecord { map, inclusion, parent, subgroup_group },
        );
        id
    }

    /// 注册群同态 G → H（已验证）。
    pub fn register_group_homomorphism(
        &mut self,
        source: GroupId,
        target: GroupId,
        source_presentation: PresentationId,
        target_presentation: PresentationId,
        element_images: HashMap<Vec<u32>, RawPerm>,
    ) -> AlgebraMapId {
        let id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id,
            source: AlgebraParentId::Group(source),
            target: AlgebraParentId::Group(target),
            kind: AlgebraMapKind::GroupHomomorphism,
            verification: MapVerification { kind: MapVerificationKind::GeneratorRelations, verified: true },
        };
        let homomorphism = GroupHomomorphism { map: id, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.homomorphisms.insert(
            id,
            GroupHomomorphismRecord { map, homomorphism, source, target, element_images },
        );
        id
    }

    /// 注册商投影 G → G/N（已验证正规）。
    pub fn register_quotient_projection(
        &mut self,
        subgroup: SubgroupId,
        parent: GroupId,
        quotient: GroupId,
        source_presentation: PresentationId,
        target_presentation: PresentationId,
    ) -> AlgebraMapId {
        if let Some(r) = self.quotient_projections.get(&subgroup) {
            return r.map.id;
        }
        let id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id,
            source: AlgebraParentId::Group(parent),
            target: AlgebraParentId::Group(quotient),
            kind: AlgebraMapKind::QuotientProjection,
            verification: MapVerification { kind: MapVerificationKind::GeneratorRelations, verified: true },
        };
        let projection = QuotientProjection {
            map: id,
            subgroup,
            source_presentation,
            target_presentation,
        };
        self.maps.insert(id, map.clone());
        self.quotient_projections.insert(
            subgroup,
            QuotientProjectionRecord { map, projection, parent, quotient },
        );
        id
    }
}
