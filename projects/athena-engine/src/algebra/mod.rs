//! 统一代数父对象内核（Living `18` Phase 0）。
//!
//! `group` / `field` / `galois` / `polynomial` 均引用本模块的类型边界，
//! 不得平行维护 parent / map / property 语义。

mod bsgs;
mod element;
mod finite_field_poly;
mod group_facts;
mod group_table;
mod map;
mod map_table;
mod parent;
mod permutation;
mod presentation;
mod property;
mod table;

pub use bsgs::BsgsChain;
pub use element::{AlgebraElement, ElementProvenance};
pub use finite_field_poly::{FiniteFieldPolySpec, add_coords, canonical_coords, inv_coords, mul_coords};
pub use group_facts::GroupPropertyFacts;
pub use group_table::{GroupTable, PermutationGroupSpec};
pub use map::{AlgebraMap, AlgebraMapKind, FieldEmbedding, GroupHomomorphism, MapVerification, MapVerificationKind};
pub use map_table::MapTable;
pub use parent::{AlgebraParentId, CoefficientParent};
pub use permutation::RawPerm;
pub use presentation::{
    FieldPresentation, FieldPresentationId, FieldPresentationKind, GroupPresentation, GroupPresentationId,
    GroupPresentationKind,
};
pub use property::{PropertyState, PropertyWitness};
pub use table::FieldTable;
