//! 语义身份与假设作用域表（Living `25` / `26`）。
//!
//! - [`crate::runtime::values::ValueStore`]：`ValueId` → [`crate::runtime::values::RuntimeValue`]
//! - [`crate::runtime::results::ResultStore`]：`ResultId` → [`crate::runtime::results::ComputationResult`]
//! - [`AssumptionScopeTable`]：[`athena_types::AssumptionScope`] 的 Session intern
//!
//! `TermId` 是 `TermStore` 原生引用。禁止 `ValueId`↔`TermId` 双射表。

mod scope_table;

pub use scope_table::AssumptionScopeTable;
