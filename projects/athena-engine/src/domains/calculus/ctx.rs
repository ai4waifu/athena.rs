//! Temporary calculus facade over [`DomainExecutionContext`] (Living `27`).
//!
//! **Delete this module** once integral / limit / series / vector / ODE / transform
//! no longer need string extension helpers. New code must use
//! [`DomainExecutionContext`] only.

#![allow(unsafe_code)]

use athena_ir::ApplicationHead;
use athena_types::TermId;
use std::ops::{Deref, DerefMut};

use crate::domains::context::DomainExecutionContext;
use crate::runtime::session::Session;

/// Calculus-local wrapper. Prefer [`DomainExecutionContext`] for new paths.
pub struct CalculusCtx<'a> {
    domain: DomainExecutionContext<'a>,
}

impl<'a> CalculusCtx<'a> {
    /// Bind session.
    pub fn new(s: &'a mut Session) -> Self {
        Self { domain: DomainExecutionContext::new(s) }
    }

    /// Extension display-name check (legacy · delete with this module).
    pub(crate) fn extension_named(&self, head: ApplicationHead, name: &str) -> bool {
        matches!(head, ApplicationHead::Extension(id) if self.domain.session().operators.name(id) == Some(name))
    }

    /// Symbol display-name equality (legacy · prefer [`SymbolId`] compare).
    pub(crate) fn symbol_is(&self, symbol: athena_types::SymbolId, name: &str) -> bool {
        self.domain.symbol_resolve(symbol) == name
    }

    /// Alias for [`DomainExecutionContext::fold_term`].
    pub(crate) fn eval(&self, id: TermId) -> TermId {
        self.domain.fold_term(id)
    }

    /// Ordered collection (legacy name · use [`DomainExecutionContext::ordered`]).
    pub(crate) fn list(&self, items: Vec<TermId>) -> TermId {
        self.domain.ordered(items)
    }

    /// Extension application by display name (legacy · delete with this module).
    pub(crate) fn apply(&self, head: &str, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_application_named(self.domain.session_mut(), head, args)
    }
}

impl<'a> Deref for CalculusCtx<'a> {
    type Target = DomainExecutionContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.domain
    }
}

impl<'a> DerefMut for CalculusCtx<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.domain
    }
}
