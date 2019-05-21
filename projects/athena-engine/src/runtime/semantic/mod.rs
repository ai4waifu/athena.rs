//! 语义身份与假设作用域表（Living 表达式 / 假设合同）。
//!
//! - [`ValueIdTable`] / [`ResultIdTable`]：值与结果容器身份分配
//! - [`AssumptionScopeTable`]：[`athena_types::AssumptionScope`] 的 Session intern
//!
//! `ExprId` 即 AthenaIR arena 原生引用（Living `25`），不再经二级映射表。

mod bindings;
mod scope_table;

pub use bindings::{ResultIdTable, ValueIdTable};
pub use scope_table::AssumptionScopeTable;
