//! Scope-local E-Graph storage (bootstrap · Living `26`).

use std::collections::HashMap;

use athena_ir::{ApplicationHead, Atom, TermNode, TermStore};
use athena_types::TermId;

use super::ids::{EClassId, ENodeId};

/// Structural enode key (operator + child class ids).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct ENodeKey {
    pub head: ApplicationHead,
    pub children: Vec<EClassId>,
}

/// One enode payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ENode {
    pub key: ENodeKey,
    /// Optional originating TermStore root (atom / app root).
    pub term: Option<TermId>,
    pub eclass: EClassId,
}

/// Scope-local equality graph for **candidate** search only.
#[derive(Debug, Default)]
pub struct EGraph {
    /// Union-find parent over e-class ids (local, not ExactUnionFind).
    parent: Vec<EClassId>,
    /// Enodes by id.
    enodes: Vec<ENode>,
    /// Hash-cons: structural key → enode.
    by_key: HashMap<ENodeKey, ENodeId>,
    /// Term root → e-class (after add).
    term_class: HashMap<TermId, EClassId>,
}

impl EGraph {
    /// Empty graph.
    pub fn new() -> Self {
        Self::default()
    }

    /// E-class count (including merged slots; use [`Self::eclass_count`] for roots).
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// Whether no classes exist.
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// Number of enodes.
    pub fn enode_count(&self) -> usize {
        self.enodes.len()
    }

    /// Number of distinct e-class roots.
    pub fn eclass_count(&self) -> usize {
        (0..self.parent.len())
            .map(|i| self.find(EClassId(i as u32)).0)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    /// Find representative e-class.
    pub fn find(&self, id: EClassId) -> EClassId {
        let mut current = id;
        loop {
            let parent = self.parent.get(current.0 as usize).copied().unwrap_or(current);
            if parent == current {
                return current;
            }
            current = parent;
        }
    }

    /// Candidate union of two classes. Does **not** touch M-Graph.
    pub fn union_classes(&mut self, left: EClassId, right: EClassId) -> bool {
        let root_left = self.find(left);
        let root_right = self.find(right);
        if root_left == root_right {
            return false;
        }
        // Prefer lower id as root (deterministic).
        let (keep, drop) = if root_left.0 <= root_right.0 {
            (root_left, root_right)
        } else {
            (root_right, root_left)
        };
        if let Some(slot) = self.parent.get_mut(drop.0 as usize) {
            *slot = keep;
        }
        true
    }

    /// Terms previously added to this graph (session-local roots).
    pub fn known_terms(&self) -> Vec<TermId> {
        self.term_class.keys().copied().collect()
    }

    /// All term roots recorded in `class` (after find).
    pub fn terms_in_class(&self, class: EClassId) -> Vec<TermId> {
        let root = self.find(class);
        let mut out = Vec::new();
        for (term, c) in &self.term_class {
            if self.find(*c) == root {
                out.push(*term);
            }
        }
        out.sort_by_key(|t| t.0);
        out.dedup();
        out
    }

    /// Lookup e-class for a previously added term.
    pub fn class_of_term(&self, term: TermId) -> Option<EClassId> {
        self.term_class.get(&term).map(|c| self.find(*c))
    }

    /// First term root recorded for an e-class (bootstrap extract).
    pub fn term_for_class(&self, class: EClassId) -> Option<TermId> {
        let root = self.find(class);
        for (term, c) in &self.term_class {
            if self.find(*c) == root {
                return Some(*term);
            }
        }
        for node in &self.enodes {
            if self.find(node.eclass) == root {
                if let Some(term) = node.term {
                    return Some(term);
                }
            }
        }
        None
    }

    /// Add a TermStore root into the graph (structural recursion).
    pub fn add_term(&mut self, store: &TermStore, term: TermId) -> Option<EClassId> {
        if let Some(existing) = self.term_class.get(&term).copied() {
            return Some(self.find(existing));
        }
        let class = match store.get(term)? {
            TermNode::Atom(atom) => self.add_atom(term, atom),
            TermNode::Collection { elements, .. } => {
                // Collections: treat as opaque atom-like leaf for bootstrap (no List semantics).
                let _ = elements;
                self.alloc_class_for_term(term)
            }
            TermNode::Application { head, arguments } => {
                let mut children = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    children.push(self.add_term(store, *arg)?);
                }
                self.add_application(term, *head, children)
            }
        };
        self.term_class.insert(term, class);
        Some(class)
    }

    fn add_atom(&mut self, term: TermId, atom: &Atom) -> EClassId {
        let _ = atom;
        self.alloc_class_for_term(term)
    }

    fn add_application(&mut self, term: TermId, head: ApplicationHead, children: Vec<EClassId>) -> EClassId {
        let key = ENodeKey { head, children };
        if let Some(id) = self.by_key.get(&key).copied() {
            let class = self.enodes[id.0 as usize].eclass;
            return self.find(class);
        }
        let class = self.alloc_class();
        let enode_id = ENodeId(self.enodes.len() as u32);
        self.enodes.push(ENode {
            key: key.clone(),
            term: Some(term),
            eclass: class,
        });
        self.by_key.insert(key, enode_id);
        class
    }

    fn alloc_class_for_term(&mut self, term: TermId) -> EClassId {
        let class = self.alloc_class();
        // Leaf enode with empty children and a sentinel extension head is avoided:
        // leaves are class-only until extractor needs structure.
        let _ = term;
        class
    }

    fn alloc_class(&mut self) -> EClassId {
        let id = EClassId(self.parent.len() as u32);
        self.parent.push(id);
        id
    }
}
