//! 域论 — ℚ / 𝔽_p / 有限扩张（骨架）。
//!
//! 元素必须绑定所属域；跨域运算 → `ATHENA_FIELD_MISMATCH`。

mod request;
mod result;
mod types;
mod value;

pub use request::FieldRequest;
pub use result::{FieldResult, execute_field};
pub use types::{Field, FieldDescriptor, FieldElement, FieldElementRepr, FieldKind};
pub use value::FieldDomainValue;
