//! 群论 — 有限群 / 置换群对象与运算（骨架）。
//!
//! 元素必须绑定所属群；跨群运算 → `ATHENA_GROUP_MISMATCH`。

mod request;
mod result;
mod types;
mod value;

pub use request::GroupRequest;
pub use result::{GroupResult, execute_group};
pub use types::{Group, GroupDescriptor, GroupElement, GroupElementRepr, Permutation};
pub use value::GroupDomainValue;
