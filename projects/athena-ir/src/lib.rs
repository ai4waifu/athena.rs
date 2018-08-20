//! athena Core CAS IR — term arena、节点、builder、验证。

#![deny(missing_docs)]

pub mod arena;
pub mod builder;
pub mod hash;
pub mod node;
pub mod symbol;

pub use arena::TermArena;
pub use builder::TermBuilder;
pub use hash::canonical_hash;
pub use node::{AtomKind, TermKind};
pub use symbol::SymbolTable;
