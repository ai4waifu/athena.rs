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
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, Default)]
pub struct ExtensionRegistry {
    names: Vec<String>,
    by_name: HashMap<String, ExtensionOperatorId>,
}

impl ExtensionRegistry {
    /// Empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Owning 复制（Living `31`：显示名表）。
    pub fn owning_copy(&self) -> Self {
        Self {
            names: self.names.clone(),
            by_name: self.by_name.clone(),
        }
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

    /// Resolve an extension operator display name (debug / renderer / diagnostics).
    ///
    /// Never use the returned string to recover [`super::SemanticOperator`] or to
    /// choose an executor / provider / domain request.
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
