//! 中性 typed TRS 模式合同。
//!
//! Mathematica `Blank` / `Pattern` 等是方言表面，不得作为内部规则本体。

mod binder;
mod pattern;

pub use binder::{PatternBindings, match_pattern, substitute};
pub use pattern::{PatternConstraint, TermPattern};
