//! Operator identities for IR applications.
//!
//! Core math / logic / structure ops use the closed [`SemanticOperator`] enum.
//! [`ExtensionRegistry`] is an **extension display-name registry only** — not a
//! core semantic catalog. Dialect surface names are injected by product layers.

mod semantic;

use std::collections::HashMap;

use athena_types::ExtensionOperatorId;

pub use semantic::{ApplicationHead, SemanticOperator, UnaryFunction};

/// Extension display-name ↔ [`ExtensionOperatorId`] bidirectional table.
///
/// Not a core operator catalog. Core ops use [`SemanticOperator`].
#[derive(Debug, Clone, Default)]
pub struct ExtensionRegistry {
    names: Vec<String>,
    by_name: HashMap<String, ExtensionOperatorId>,
}

impl ExtensionRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Allocate or look up an extension operator id.
    pub fn intern(&mut self, name: &str) -> ExtensionOperatorId {
        if let Some(id) = self.by_name.get(name) {
            return *id;
        }
        let id = ExtensionOperatorId(self.names.len() as u32);
        self.names.push(name.to_string());
        self.by_name.insert(name.to_string(), id);
        id
    }

    /// Look up an existing id by display name without allocating.
    ///
    /// Boundary / product-layer helper only. Core execution must never branch on
    /// the returned name or use this to recover [`super::SemanticOperator`].
    pub fn lookup_display_name(&self, name: &str) -> Option<ExtensionOperatorId> {
        self.by_name.get(name).copied()
    }

    /// Resolve an extension operator display name (debug / renderer / diagnostics).
    pub fn display_name(&self, id: ExtensionOperatorId) -> Option<&str> {
        self.names.get(id.0 as usize).map(String::as_str)
    }

    /// Registered extension count.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}
