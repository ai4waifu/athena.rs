//! Typed rewrite rules that feed E-Graph candidate search (Living `03` R-2.2 / `26`).
//!
//! Rules here are **internal** contracts — not dialect `ReplacementRule` / Blank patterns.
//! Emitting a match never admits M-Graph facts.

use athena_types::TermId;

/// Stable rewrite rule identity within a [`RuleSet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RewriteRuleId(pub u32);

/// One compiled rewrite rule (pattern → replacement template).
///
/// Pattern / replacement payloads stay as host [`TermId`] roots for bootstrap.
/// Full [`athena_engine::reasoning::trs::TermPattern`] integration lands with the engine
/// saturation rule matcher (engine depends on rewriter, not the reverse).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RewriteRule {
    /// Rule identity.
    pub id: RewriteRuleId,
    /// Left-hand pattern term root (structure-only until matcher lands).
    pub pattern: TermId,
    /// Right-hand replacement template root.
    pub replacement: TermId,
    /// Optional human debug label (not a dispatch key).
    pub debug_label: Option<&'static str>,
}

/// Ordered collection of rewrite rules for one saturation scope.
#[derive(Debug, Default, Clone)]
pub struct RuleSet {
    rules: Vec<RewriteRule>,
    next_id: u32,
}

impl RuleSet {
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

    /// Register a pattern → replacement pair.
    pub fn push(&mut self, pattern: TermId, replacement: TermId, debug_label: Option<&'static str>) -> RewriteRuleId {
        let id = RewriteRuleId(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        self.rules.push(RewriteRule {
            id,
            pattern,
            replacement,
            debug_label,
        });
        id
    }

    /// Iterate rules in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &RewriteRule> {
        self.rules.iter()
    }

    /// Lookup by id.
    pub fn get(&self, id: RewriteRuleId) -> Option<&RewriteRule> {
        self.rules.iter().find(|r| r.id == id)
    }
}

/// Local rewrite witness (conditions / provenance filled later).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalRewriteWitness {
    /// Rule that fired.
    pub rule: RewriteRuleId,
    /// Matched subject term.
    pub subject: TermId,
    /// Produced term.
    pub produced: TermId,
}
