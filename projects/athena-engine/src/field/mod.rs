//! 域论 — ℚ / 𝔽_p / 有限扩张（骨架）。
//!
//! 元素必须绑定所属域；跨域运算 → `ATHENA_FIELD_MISMATCH`。

mod canonical;
mod request;
mod result;
mod types;
mod value;

pub use canonical::{
    add_field_elements, apply_field_embedding, canonical_prime_residue, canonical_rational, inv_field_element,
    mul_field_elements,
};
pub use request::FieldRequest;
pub use result::{FieldResult, execute_field, execute_field_with_table, execute_field_with_table_mut};
pub use types::{Field, FieldDescriptor, FieldElement, FieldElementRepr, FieldKind};
pub use value::FieldDomainValue;
