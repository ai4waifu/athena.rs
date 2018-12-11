//! 群论 — 有限群 / 置换群对象与运算（骨架）。
//!
//! 元素必须绑定所属群；跨群运算 → `ATHENA_GROUP_MISMATCH`。

mod canonical;
mod request;
mod result;
mod types;
mod value;

pub use canonical::{
    apply_group_homomorphism, canonical_permutation, group_membership, inverse_group_element, multiply_group_elements,
    project_quotient_element,
};
pub use request::GroupRequest;
pub use result::{GroupResult, execute_group, execute_group_with_table, execute_group_with_table_mut};
pub use types::{Group, GroupDescriptor, GroupElement, GroupElementRepr, GroupKind, Permutation, Subgroup};
pub use value::GroupDomainValue;
