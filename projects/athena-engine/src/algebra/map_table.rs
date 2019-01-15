//! 代数映射注册表（canonical embedding、子群包含、同态、商投影）。

use std::collections::HashMap;

use athena_types::{
    AlgebraMapId, AutomorphismId, ExtensionId, FieldId, FieldPresentationId, GroupId, GroupPresentationId, SubgroupId,
};

use super::{
    map::{
        AlgebraMap, AlgebraMapKind, FieldEmbedding, GroupHomomorphism, MapVerification, MapVerificationKind,
        QuotientProjection, SubgroupInclusion,
    },
    parent::AlgebraParentId,
    permutation::RawPerm,
    property::PropertyWitness,
};

#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone, PartialEq)]
struct SubgroupInclusionRecord {
    map: AlgebraMap,
    inclusion: SubgroupInclusion,
    parent: GroupId,
    subgroup_group: GroupId,
}

#[derive(Debug, Clone, PartialEq)]
struct QuotientProjectionRecord {
    map: AlgebraMap,
    projection: QuotientProjection,
    parent: GroupId,
    quotient: GroupId,
}

#[derive(Debug, Clone, PartialEq)]
struct PrimeSubfieldEmbeddingRecord {
    map: AlgebraMap,
    embedding: FieldEmbedding,
    prime_field: FieldId,
    extension: FieldId,
}

#[derive(Debug, Clone, PartialEq)]
struct FieldAutomorphismRecord {
    map: AlgebraMap,
    extension: ExtensionId,
    field: FieldId,
    frobenius_power: u32,
}

/// Session 级映射表。
#[derive(Debug, Default)]
pub struct MapTable {
    next_id: u32,
    next_automorphism_id: u32,
    maps: HashMap<AlgebraMapId, AlgebraMap>,
    embeddings: HashMap<(FieldId, FieldId), FieldEmbeddingRecord>,
    prime_subfield_embeddings: HashMap<(FieldId, FieldId), PrimeSubfieldEmbeddingRecord>,
    automorphisms: HashMap<AutomorphismId, FieldAutomorphismRecord>,
    extension_automorphisms: HashMap<ExtensionId, Vec<AutomorphismId>>,
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

    /// 查 canonical embedding（源 → 靶）。
    pub fn canonical_embedding(&self, source: FieldId, target: FieldId) -> Option<AlgebraMapId> {
        self.embeddings.get(&(source, target)).map(|r| r.map.id)
    }

    /// 查域嵌入记录。
    pub fn field_embedding(&self, id: AlgebraMapId) -> Option<&FieldEmbedding> {
        self.embeddings
            .values()
            .find(|r| r.map.id == id)
            .map(|r| &r.embedding)
            .or_else(|| self.prime_subfield_embeddings.values().find(|r| r.map.id == id).map(|r| &r.embedding))
    }

    /// Frobenius 幂次（域自同构）。
    pub fn automorphism_frobenius_power(&self, id: AutomorphismId) -> Option<u32> {
        self.automorphisms.get(&id).map(|r| r.frobenius_power)
    }

    /// 自同构底层 [`AlgebraMapId`]。
    pub fn automorphism_map(&self, id: AutomorphismId) -> Option<AlgebraMapId> {
        self.automorphisms.get(&id).map(|r| r.map.id)
    }

    /// 扩张上已注册自同构 id。
    pub fn extension_automorphisms(&self, extension: ExtensionId) -> &[AutomorphismId] {
        self.extension_automorphisms.get(&extension).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// 素子域嵌入是否为已注册 map。
    pub fn is_prime_subfield_embedding(&self, id: AlgebraMapId) -> bool {
        self.prime_subfield_embeddings.values().any(|r| r.map.id == id)
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

    /// 商投影底层 [`AlgebraMap`]（供 `require_proven` 使用）。
    pub fn quotient_projection_map(&self, subgroup: SubgroupId) -> Option<&AlgebraMap> {
        self.quotient_projections.get(&subgroup).map(|r| &r.map)
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

    /// 注册已验证域嵌入 K → L（幂等；含数域基域包含）。
    pub fn register_field_embedding(
        &mut self,
        source: FieldId,
        target: FieldId,
        source_presentation: FieldPresentationId,
        target_presentation: FieldPresentationId,
    ) -> AlgebraMapId {
        self.register_canonical_q_to_fp(source, target, source_presentation, target_presentation)
    }

    /// 注册 ℚ → 𝔽_p canonical embedding（幂等）。
    pub fn register_canonical_q_to_fp(
        &mut self,
        source: FieldId,
        target: FieldId,
        source_presentation: FieldPresentationId,
        target_presentation: FieldPresentationId,
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
            verification: MapVerification::proven(
                MapVerificationKind::DegreeCheck,
                PropertyWitness::placeholder("degree_check"),
            ),
        };
        let embedding = FieldEmbedding { map: id, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.embeddings.insert((source, target), FieldEmbeddingRecord { map, embedding });
        id
    }

    /// 注册 𝔽_p ↪ 𝔽_{p^n} 素子域包含（幂等）。
    pub fn register_prime_subfield_embedding(
        &mut self,
        prime_field: FieldId,
        extension: FieldId,
        source_presentation: FieldPresentationId,
        target_presentation: FieldPresentationId,
    ) -> AlgebraMapId {
        if let Some(r) = self.prime_subfield_embeddings.get(&(prime_field, extension)) {
            return r.map.id;
        }
        let id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id,
            source: AlgebraParentId::Field(prime_field),
            target: AlgebraParentId::Field(extension),
            kind: AlgebraMapKind::FieldEmbedding,
            verification: MapVerification::proven(
                MapVerificationKind::DegreeCheck,
                PropertyWitness::placeholder("degree_check"),
            ),
        };
        let embedding = FieldEmbedding { map: id, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.prime_subfield_embeddings
            .insert((prime_field, extension), PrimeSubfieldEmbeddingRecord { map, embedding, prime_field, extension });
        id
    }

    /// 注册 Frobenius 自同构 σ^k（L → L，固定基域）。
    pub fn register_frobenius_automorphism(
        &mut self,
        extension: ExtensionId,
        field: FieldId,
        presentation: FieldPresentationId,
        frobenius_power: u32,
    ) -> AutomorphismId {
        if let Some(existing) = self.extension_automorphisms.get(&extension).and_then(|ids| {
            ids.iter().find_map(|id| {
                let rec = self.automorphisms.get(id)?;
                (rec.frobenius_power == frobenius_power).then_some(*id)
            })
        }) {
            return existing;
        }
        let id = AutomorphismId(self.next_automorphism_id);
        self.next_automorphism_id = self.next_automorphism_id.wrapping_add(1);
        let map_id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id: map_id,
            source: AlgebraParentId::Field(field),
            target: AlgebraParentId::Field(field),
            kind: AlgebraMapKind::FieldEmbedding,
            verification: MapVerification::proven(
                MapVerificationKind::GeneratorRelations,
                PropertyWitness::placeholder("generator_relations"),
            ),
        };
        let embedding = FieldEmbedding { map: map_id, source_presentation: presentation, target_presentation: presentation };
        self.maps.insert(map_id, map.clone());
        self.embeddings.insert((field, field), FieldEmbeddingRecord { map: map.clone(), embedding });
        self.automorphisms.insert(id, FieldAutomorphismRecord { map, extension, field, frobenius_power });
        self.extension_automorphisms.entry(extension).or_default().push(id);
        id
    }

    /// 注册子群包含 H ↪ G。
    pub fn register_subgroup_inclusion(
        &mut self,
        subgroup: SubgroupId,
        subgroup_group: GroupId,
        parent: GroupId,
        source_presentation: GroupPresentationId,
        target_presentation: GroupPresentationId,
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
            verification: MapVerification::proven(
                MapVerificationKind::DegreeCheck,
                PropertyWitness::placeholder("degree_check"),
            ),
        };
        let inclusion = SubgroupInclusion { map: id, subgroup, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.subgroup_inclusions.insert(subgroup, SubgroupInclusionRecord { map, inclusion, parent, subgroup_group });
        id
    }

    /// 注册群同态 G → H（已验证）。
    pub fn register_group_homomorphism(
        &mut self,
        source: GroupId,
        target: GroupId,
        source_presentation: GroupPresentationId,
        target_presentation: GroupPresentationId,
        element_images: HashMap<Vec<u32>, RawPerm>,
    ) -> AlgebraMapId {
        let id = AlgebraMapId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        let map = AlgebraMap {
            id,
            source: AlgebraParentId::Group(source),
            target: AlgebraParentId::Group(target),
            kind: AlgebraMapKind::GroupHomomorphism,
            verification: MapVerification::proven(
                MapVerificationKind::GeneratorRelations,
                PropertyWitness::placeholder("generator_relations"),
            ),
        };
        let homomorphism = GroupHomomorphism { map: id, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.homomorphisms.insert(id, GroupHomomorphismRecord { map, homomorphism, source, target, element_images });
        id
    }

    /// 注册商投影 G → G/N（已验证正规）。
    pub fn register_quotient_projection(
        &mut self,
        subgroup: SubgroupId,
        parent: GroupId,
        quotient: GroupId,
        source_presentation: GroupPresentationId,
        target_presentation: GroupPresentationId,
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
            verification: MapVerification::proven(
                MapVerificationKind::GeneratorRelations,
                PropertyWitness::placeholder("generator_relations"),
            ),
        };
        let projection = QuotientProjection { map: id, subgroup, source_presentation, target_presentation };
        self.maps.insert(id, map.clone());
        self.quotient_projections.insert(subgroup, QuotientProjectionRecord { map, projection, parent, quotient });
        id
    }
}
