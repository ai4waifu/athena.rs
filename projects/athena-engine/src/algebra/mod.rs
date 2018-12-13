//! 统一代数父对象内核：域/群/伽罗瓦/多项式共享的 parent、map、presentation 边界。
//!
//! `group` / `field` / `galois` / `polynomial` 均引用本模块的类型边界，
//! 不得平行维护 parent / map / property 语义。

mod bsgs;
mod element;
mod extension;
mod finite_field_poly;
mod galois_field;
mod group_facts;
mod group_table;
mod map;
mod map_table;
mod parent;
mod permutation;
mod presentation;
mod property;
mod subgroup;
mod table;

pub use bsgs::BsgsChain;
pub use element::{AlgebraElement, ElementProvenance};
pub use extension::FieldExtension;
pub use finite_field_poly::{
    FiniteFieldPolySpec, add_coords, canonical_coords, frobenius_coords, frobenius_power_coords, inv_coords, mul_coords,
};
pub use galois_field::{
    apply_frobenius_coords, field_automorphism, galois_group_of_extension, is_extension_normal, is_extension_separable,
    is_galois_extension,
};
pub use group_facts::GroupPropertyFacts;
pub use group_table::{GroupTable, PermutationGroupSpec};
pub use map::{
    AlgebraMap, AlgebraMapKind, FieldEmbedding, GroupHomomorphism, MapVerification, MapVerificationKind, QuotientProjection,
    SubgroupInclusion,
};
pub use map_table::MapTable;
pub use parent::{AlgebraParentId, CoefficientParent};
pub use permutation::RawPerm;
pub use presentation::{
    FieldPresentation, FieldPresentationId, FieldPresentationKind, GroupPresentation, GroupPresentationId,
    GroupPresentationKind,
};
pub use property::{PropertyState, PropertyWitness};
pub use subgroup::{coset_representatives, is_normal, quotient_generators};
pub use table::FieldTable;
