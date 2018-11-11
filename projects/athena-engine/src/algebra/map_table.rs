//! 代数映射注册表（canonical embedding 等）。

use std::collections::HashMap;

use athena_types::{AlgebraMapId, FieldId, PresentationId};

use super::map::{AlgebraMap, AlgebraMapKind, FieldEmbedding, MapVerification, MapVerificationKind};
use super::parent::AlgebraParentId;

#[derive(Debug, Clone, PartialEq, Eq)]
struct FieldEmbeddingRecord {
    map: AlgebraMap,
    embedding: FieldEmbedding,
}

/// Session 级映射表。
#[derive(Debug, Default)]
pub struct MapTable {
    next_id: u32,
    maps: HashMap<AlgebraMapId, AlgebraMap>,
    embeddings: HashMap<(FieldId, FieldId), FieldEmbeddingRecord>,
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
            verification: MapVerification {
                kind: MapVerificationKind::DegreeCheck,
                verified: true,
            },
        };
        let embedding = FieldEmbedding {
            map: id,
            source_presentation,
            target_presentation,
        };
        self.maps.insert(id, map.clone());
        self.embeddings.insert((source, target), FieldEmbeddingRecord { map, embedding });
        id
    }
}
