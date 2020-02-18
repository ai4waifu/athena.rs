//! 统一代数父对象内核：域/群/伽罗瓦/多项式共享的 parent、map、presentation 边界。
//!
//! `group` / `field` / `galois` / `polynomial` 均引用本模块的类型边界，
//! 不得平行维护 parent / map / property 语义。

mod bsgs;
mod element;
mod extension;
mod fingerprint;
mod finite_field_poly;
mod galois_field;
mod group_facts;
mod group_table;
mod map;
mod map_table;
mod number_field;
mod parent;
mod permutation;
mod presentation;
mod property;
mod subgroup;
mod table;

pub use athena_types::{FieldPresentationId, GroupPresentationId};
pub use bsgs::BsgsChain;
pub use element::{AlgebraElement, ElementProvenance};
pub use extension::FieldExtension;
pub use fingerprint::{FieldFingerprint, GroupFingerprint};
pub use finite_field_poly::{
    FiniteFieldPolySpec, add_coords, canonical_coords, frobenius_coords, frobenius_power_coords, inv_coords, mul_coords,
};
pub use galois_field::{
    apply_frobenius_coords, field_automorphism, galois_group_of_extension, is_extension_normal, is_extension_separable, is_galois_extension,
};
pub use group_facts::GroupPropertyFacts;
pub(crate) use group_facts::owning_copy_integer_property;
pub use group_table::{GroupTable, PermutationGroupSpec};
pub use map::{
    AlgebraMap, AlgebraMapKind, FieldEmbedding, GroupHomomorphism, MapVerification, MapVerificationKind, QuotientProjection, SubgroupInclusion,
};
pub use map_table::MapTable;
pub use number_field::{
    NumberFieldSpec, absolute_degree_product, add_nf_coords, canonical_nf_coords, embed_base_coords, inv_nf_coords, inv_relative_nf_coords,
    is_irreducible_over_rationals, make_monic, minimal_polynomial_from_powers, minimal_polynomial_over_q, mul_nf_coords,
    mul_relative_nf_coords, relative_modulus_from_rational, validate_rational_modulus,
};
pub use parent::{AlgebraParentId, CoefficientParent};
pub use permutation::RawPerm;
pub use presentation::{FieldPresentation, FieldPresentationKind, GroupPresentation, GroupPresentationKind};
pub use property::{PropertyState, PropertyWitness};
pub use subgroup::{coset_representatives, is_normal, quotient_generators};
pub use table::FieldTable;
