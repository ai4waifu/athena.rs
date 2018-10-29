//! 统一代数父对象内核（Living `18` Phase 0）。
//!
//! `group` / `field` / `galois` / `polynomial` 均引用本模块的类型边界，
//! 不得平行维护 parent / map / property 语义。

mod element;
mod group_facts;
mod map;
mod parent;
mod presentation;
mod property;
mod table;

pub use element::{AlgebraElement, ElementProvenance};
pub use group_facts::GroupPropertyFacts;
pub use map::{AlgebraMap, AlgebraMapKind, FieldEmbedding, GroupHomomorphism, MapVerification, MapVerificationKind};
pub use parent::{AlgebraParentId, CoefficientParent};
pub use presentation::{
    FieldPresentation, FieldPresentationId, FieldPresentationKind, GroupPresentation, GroupPresentationId,
    GroupPresentationKind,
};
pub use property::{PropertyState, PropertyWitness};
pub use table::FieldTable;
