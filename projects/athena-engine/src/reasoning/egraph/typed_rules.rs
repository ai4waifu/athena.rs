//! Typed [`TermPattern`] rules for E-Graph saturation (Living `26` / `27`).
//!
//! Pattern/match/substitute ownership is [`athena_rewriter`]. This module only
//! packages rules for engine-local saturation (never admits M-Graph facts).

use athena_rewriter::{RewriteRuleId, TermPattern};
use athena_types::TermId;

/// One typed rewrite rule (pattern → replacement template).
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct TypedRewriteRule {
    /// Rule identity (same id space as [`athena_rewriter::RuleSet`] when mixed carefully).
    pub id: RewriteRuleId,
    /// Left-hand neutral pattern.
    pub pattern: TermPattern,
    /// Right-hand replacement template (may contain bound symbols).
    pub replacement: TermId,
    /// Optional human debug label (not a dispatch key).
    pub debug_label: Option<&'static str>,
}

impl TypedRewriteRule {
    /// Owning 复制（Living `31`：经 [`TermPattern::owning_copy`]）。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            pattern: self.pattern.owning_copy(),
            replacement: self.replacement,
            debug_label: self.debug_label,
        }
    }
}

/// Ordered collection of typed rewrite rules for one saturation scope.
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default)]
pub struct TypedRuleSet {
    rules: Vec<TypedRewriteRule>,
    next_id: u32,
}

impl TypedRuleSet {
    /// Owning 复制（Living `31`：经 [`TypedRewriteRule::owning_copy`]）。
    pub fn owning_copy(&self) -> Self {
        Self {
            rules: self.rules.iter().map(TypedRewriteRule::owning_copy).collect(),
            next_id: self.next_id,
        }
    }

    /// Empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Number of rules.
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Register a typed pattern → replacement template.
    pub fn push(&mut self, pattern: TermPattern, replacement: TermId, debug_label: Option<&'static str>) -> RewriteRuleId {
        let id = RewriteRuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.rules.push(TypedRewriteRule { id, pattern, replacement, debug_label });
        id
    }

    /// Iterate rules in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &TypedRewriteRule> {
        self.rules.iter()
    }

    /// Lookup by id.
    pub fn get(&self, id: RewriteRuleId) -> Option<&TypedRewriteRule> {
        self.rules.iter().find(|r| r.id == id)
    }
}
