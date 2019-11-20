//! 求值环境：定义层与作用域帧。

pub mod compiled_rules;
pub mod definitions;

pub use compiled_rules::CompiledRuleStore;
pub use definitions::{DefinitionLayer, LocalBinding, ScopeFrame};
