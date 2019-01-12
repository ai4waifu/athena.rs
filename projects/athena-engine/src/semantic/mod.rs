//! Living `24` SEM0/SEM1：语义身份绑定与假设作用域表。
//!
//! - [`ExprBindingTable`]：[`athena_types::ExprId`] ↔ 存储 [`athena_types::TermId`]
//! - [`ValueIdTable`] / [`ResultIdTable`]：值与结果容器身份分配
//! - [`AssumptionScopeTable`]：[`athena_types::AssumptionScope`] Session intern

mod bindings;
mod scope_table;

pub use bindings::{ExprBindingTable, ResultIdTable, ValueIdTable};
pub use scope_table::AssumptionScopeTable;
