//! athena Core CAS IR — term arena、节点、builder、验证。
//!
//! 数字原子持有 [`athena_numeric::NumericValue`]。

#![deny(missing_docs)]

pub mod arena;
pub mod build;
pub mod canonical;
pub mod node;
pub mod operator;
pub mod symbol;
pub mod trace;

/// 数值塔再导出。
pub use athena_numeric as numeric;

pub use arena::TermStore;
pub use build::TermBuilder;
pub use canonical::{canonical_hash, canonical_hash_named, fnv1a64};
pub use node::{Atom, TermNode};
pub use operator::OperatorRegistry;
pub use symbol::SymbolTable;
pub use trace::trace_term_node;
