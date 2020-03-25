//! 域扩张 K ↪ L 与塔链接。

use athena_types::{AlgebraMapId, ExtensionId, FieldId};

use super::property::{PropertyState, PropertyWitness};

/// 域扩张 L/K（含已验证嵌入 K → L）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct FieldExtension {
    /// 稳定扩张 id。
    pub id: ExtensionId,
    /// 基域 K。
    pub base: FieldId,
    /// 扩张域 L。
    pub field: FieldId,
    /// 扩张次数 [L:K]。
    pub degree: PropertyState<u32>,
    /// 可分性。
    pub separable: PropertyState<bool>,
    /// 正规性。
    pub normal: PropertyState<bool>,
    /// 包含嵌入 K ↪ L。
    pub embedding: AlgebraMapId,
}

impl FieldExtension {
    /// 构造已证明次数的有限域多项式基扩张记录。
    pub fn finite_field_polynomial(id: ExtensionId, base: FieldId, field: FieldId, degree: u32, embedding: AlgebraMapId) -> Self {
        let witness = PropertyWitness::placeholder("finite_field_polynomial_basis");
        Self {
            id,
            base,
            field,
            degree: PropertyState::Proven { value: degree, witness: witness.owning_copy() },
            separable: PropertyState::Proven { value: true, witness: PropertyWitness::placeholder("char_p_separable") },
            normal: PropertyState::Proven { value: true, witness: PropertyWitness::placeholder("finite_field_normal") },
            embedding,
        }
    }

    /// 构造数域幂基 / 相对塔扩张记录。
    pub fn number_field(id: ExtensionId, base: FieldId, field: FieldId, degree: u32, embedding: AlgebraMapId, separable: bool) -> Self {
        Self {
            id,
            base,
            field,
            degree: PropertyState::Proven { value: degree, witness: PropertyWitness::placeholder("number_field_defining_polynomial") },
            separable: PropertyState::Proven { value: separable, witness: PropertyWitness::placeholder("char0_separable_irreducible") },
            normal: PropertyState::Unknown,
            embedding,
        }
    }

    /// 已证明次数（若可用）。
    pub fn proven_degree(&self) -> Option<u32> {
        match &self.degree {
            PropertyState::Proven { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            base: self.base,
            field: self.field,
            degree: self.degree.owning_copy(),
            separable: self.separable.owning_copy(),
            normal: self.normal.owning_copy(),
            embedding: self.embedding,
        }
    }
}

/// 沿 base 链自素域（或链顶基域）到 L 的域 id 塔（升序）。
pub fn extension_tower_fields<'a>(
    extension: &FieldExtension,
    resolve_field_extension: impl Fn(FieldId) -> Option<&'a FieldExtension>,
) -> Vec<FieldId> {
    let mut chain = vec![extension.base, extension.field];
    let mut cursor = extension.base;
    while let Some(parent) = resolve_field_extension(cursor) {
        if chain.first() == Some(&parent.base) {
            break;
        }
        chain.insert(0, parent.base);
        cursor = parent.base;
    }
    chain
}
