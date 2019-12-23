//! Typed [`TermPattern`] rules for E-Graph saturation (Living `26` / `27`).
//!
//! Lives in `athena-engine` because [`TermPattern`] is an engine TRS contract.
//! Emitting matches never admits M-Graph facts.

use athena_rewriter::RewriteRuleId;

use crate::reasoning::trs::TermPattern;
use athena_types::TermId;

/// One typed rewrite rule (pattern → replacement template).
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Ordered collection of typed rewrite rules for one saturation scope.
#[derive(Debug, Default, Clone)]
pub struct TypedRuleSet {
    rules: Vec<TypedRewriteRule>,
    next_id: u32,
}

impl TypedRuleSet {
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
    pub fn push(
        &mut self,
        pattern: TermPattern,
        replacement: TermId,
        debug_label: Option<&'static str>,
    ) -> RewriteRuleId {
        let id = RewriteRuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.rules.push(TypedRewriteRule {
            id,
            pattern,
            replacement,
            debug_label,
        });
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
