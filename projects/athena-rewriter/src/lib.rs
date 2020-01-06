//! 面向 Core IR 的改写器（规则、规范化、局部 match/substitute · Living `00`/`26`/`27`）。
//!
//! **本 crate 拥有**：[`TermPattern`]、[`match_pattern`]、[`substitute`]、[`RuleSet`]、局部 witness。
//! **不拥有**：E-Graph lifecycle、saturation budget、AdmissionGate、M-Graph（均在 `athena-engine`）。

#![deny(missing_docs)]

mod binder;
mod engine;
mod pattern;
mod rule;

pub use binder::{PatternBindings, match_pattern, substitute};
pub use engine::{RewriteOptions, RewriteResult, Rewriter};
pub use pattern::{PatternConstraint, TermPattern};
pub use rule::{LocalRewriteWitness, RewriteRule, RewriteRuleId, RuleSet};
