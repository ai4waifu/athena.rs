//! 中性 typed TRS 模式合同。
//!
//! Mathematica `Blank` / `Pattern` 等是方言表面，不得作为内部规则本体。

mod pattern;

pub use pattern::{PatternConstraint, TermPattern};
