//! 值存储、arena 操作与 term 访问。

pub mod arena;
pub mod numeric_clone;
pub mod store;
pub mod term_access;

pub use store::{RuntimeValue, ValueStore};
