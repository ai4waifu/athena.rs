//! athena Core CAS IR — term arena、节点、builder、验证。
//!
//! 依赖 `athena-numeric`（N0：链路接线；值持有迁移后续完成）。

#![deny(missing_docs)]

pub mod arena;
pub mod builder;
pub mod hash;
pub mod node;
pub mod symbol;

/// 数值塔再导出（便于 IR 层后续持有 `NumericValue`）。
pub use athena_numeric as numeric;

pub use arena::TermArena;
pub use builder::TermBuilder;
pub use hash::canonical_hash;
pub use node::{AtomKind, TermKind};
pub use symbol::SymbolTable;
