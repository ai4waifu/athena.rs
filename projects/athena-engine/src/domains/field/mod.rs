#![doc = include_str!("readme.md")]

//! 域论 — ℚ / 𝔽_p / 有限扩张 / 数域。
//!
//! 已实现：ℚ / 𝔽_p / 𝔽_{p^n} / `ℚ(α)` canonical 运算与相对扩张塔。
//!
//! 元素必须绑定所属域；跨域运算 → `ATHENA_FIELD_MISMATCH`。

mod canonical;
mod request;
mod result;
mod types;
mod value;

pub use canonical::{
    add_field_elements, apply_base_field_embedding, apply_field_automorphism, apply_field_embedding, apply_prime_subfield_embedding,
    canonical_extension_element, canonical_number_field_element, canonical_prime_residue, canonical_rational, inv_field_element,
    mul_field_elements,
};
pub use request::FieldRequest;
pub use result::{FieldResult, execute_field, execute_field_with_table, execute_field_with_table_mut};
pub use types::{Field, FieldDescriptor, FieldElement, FieldElementRepr};
pub use value::FieldDomainValue;
