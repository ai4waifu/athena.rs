//! Shared domain execution capabilities (Living `27`).
//!
//! All domains that need term read/build share this context. It is **not** a
//! calculus-specific mini-evaluator: no string head apply, no extension display-name
//! dispatch, no symbol-name operator guessing.

#![allow(unsafe_code)]

use athena_ir::{ApplicationHead, Atom, SemanticOperator};
use athena_numeric::Number;
use athena_types::{CollectionKind, OperatorId, SymbolId, TermId};
use std::marker::PhantomData;

use crate::{
    api::request::AthenaRequest,
    execution,
    execution::shape::Shape,
    runtime::{session::Session, values::numeric_clone::clone_number},
};

/// Pre-interned residual extension identities (compare by [`OperatorId`], never by display name).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResidualExtensionIds {
    /// Residual indeterminate form marker.
    pub indeterminate: OperatorId,
    /// Real-part residual used in ROC predicates.
    pub re: OperatorId,
    /// Element-of residual used in ROC / domain predicates.
    pub element: OperatorId,
    /// Unit step / Heaviside causal marker.
    pub unit_step: OperatorId,
    /// Alternate Heaviside name (same semantic residual family).
    pub heaviside_theta: OperatorId,
    /// Kronecker delta residual.
    pub kronecker_delta: OperatorId,
    /// Discrete delta residual.
    pub discrete_delta: OperatorId,
}

/// Shared term read/build capability for domain providers.
pub struct DomainExecutionContext<'a> {
    s: *mut Session,
    ext: ResidualExtensionIds,
    _marker: PhantomData<&'a mut Session>,
}

impl<'a> DomainExecutionContext<'a> {
    /// Bind an exclusive session borrow for the duration of a domain call.
    pub fn new(s: &'a mut Session) -> Self {
        let ext = ResidualExtensionIds {
            indeterminate: s.operators.intern("Indeterminate"),
            re: s.operators.intern("Re"),
            element: s.operators.intern("Element"),
            unit_step: s.operators.intern("UnitStep"),
            heaviside_theta: s.operators.intern("HeavisideTheta"),
            kronecker_delta: s.operators.intern("KroneckerDelta"),
            discrete_delta: s.operators.intern("DiscreteDelta"),
        };
        Self { s: s as *mut Session, ext, _marker: PhantomData }
    }

    /// Residual extension id table for this session.
    pub(crate) fn residual_extensions(&self) -> ResidualExtensionIds {
        self.ext
    }

    /// Whether `head` is the pre-interned Indeterminate residual.
    pub(crate) fn is_indeterminate_extension(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Extension(id) if id == self.ext.indeterminate)
    }

    /// UnitStep or HeavisideTheta residual head.
    pub(crate) fn is_unit_step_extension(&self, head: ApplicationHead) -> bool {
        matches!(
            head,
            ApplicationHead::Extension(id) if id == self.ext.unit_step || id == self.ext.heaviside_theta
        )
    }

    /// KroneckerDelta or DiscreteDelta residual head.
    pub(crate) fn is_delta_extension(&self, head: ApplicationHead) -> bool {
        matches!(
            head,
            ApplicationHead::Extension(id) if id == self.ext.kronecker_delta || id == self.ext.discrete_delta
        )
    }

    /// Element residual head.
    pub(crate) fn is_element_extension(&self, head: ApplicationHead) -> bool {
        matches!(head, ApplicationHead::Extension(id) if id == self.ext.element)
    }

    /// Intern an extension operator id (ODE dependent head etc. · not core math).
    pub(crate) fn intern_extension(&self, name: &str) -> OperatorId {
        self.session_mut().operators.intern(name)
    }

    /// Extension application by [`OperatorId`].
    pub(crate) fn apply_extension(&self, id: OperatorId, args: Vec<TermId>) -> TermId {
        self.apply_head(ApplicationHead::Extension(id), args)
    }

    #[inline]
    pub(crate) fn session(&self) -> &Session {
        // SAFETY: lifetime exclusivity matches former CalculusCtx invariant.
        unsafe { &*self.s }
    }

    #[inline]
    pub(crate) fn session_mut(&self) -> &mut Session {
        // SAFETY: serial builder / fold use; no overlapping `&mut Session`.
        unsafe { &mut *self.s }
    }

    /// Cheap structural snapshot (does not clone numeric payloads).
    pub(crate) fn shape(&self, id: TermId) -> Option<Shape> {
        match self.session().arena.get(id)? {
            athena_ir::TermNode::Atom(Atom::Number(_)) => Some(Shape::Number),
            athena_ir::TermNode::Atom(Atom::String(v)) => Some(Shape::String(v.clone())),
            athena_ir::TermNode::Atom(Atom::Symbol(s)) => Some(Shape::Symbol(*s)),
            athena_ir::TermNode::Atom(Atom::Boolean(b)) => Some(Shape::Bool(*b)),
            athena_ir::TermNode::Atom(Atom::Null) => Some(Shape::Null),
            athena_ir::TermNode::Collection { elements: items, .. } => Some(Shape::Collection(items.clone())),
            athena_ir::TermNode::Application { head: op, arguments: args } => Some(Shape::Application(*op, args.clone())),
        }
    }

    /// Typed application head + arguments.
    pub(crate) fn application_head(&self, id: TermId) -> Option<(ApplicationHead, Vec<TermId>)> {
        match self.shape(id)? {
            Shape::Application(op, args) => Some((op, args)),
            _ => None,
        }
    }

    /// Arena number reference.
    pub(crate) fn number_of(&self, id: TermId) -> Option<&Number> {
        match self.session().arena.get(id) {
            Some(athena_ir::TermNode::Atom(Atom::Number(n))) => Some(n),
            _ => None,
        }
    }

    /// Integer exponent when the atom is an exact small integer.
    pub(crate) fn int_exp(&self, id: TermId) -> Option<i64> {
        self.number_of(id).and_then(|n| n.as_integer_exp())
    }

    /// Owning numeric copy for portable fold paths.
    pub(crate) fn copy(&self, n: &Number) -> Number {
        clone_number(n)
    }

    /// Structural equality in the session arena.
    pub(crate) fn eq(&self, a: TermId, b: TermId) -> bool {
        self.session().arena.structural_eq(a, b)
    }

    /// Whether a symbol atom equals the given [`SymbolId`].
    pub(crate) fn symbol_id_is(&self, symbol: SymbolId, expected: SymbolId) -> bool {
        symbol == expected
    }

    /// Fold via the sole `ExecutionIR` path (explicit term request, never string heads).
    pub(crate) fn fold_term(&self, id: TermId) -> TermId {
        match execution::execute_ir_request(self.session_mut(), AthenaRequest::Term(id)) {
            Ok(result_id) => self.session().results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(id),
            Err(_) => id,
        }
    }

    /// Number atom.
    pub(crate) fn num(&self, n: Number) -> TermId {
        execution::push_number(self.session_mut(), n)
    }

    /// Exact small integer.
    pub(crate) fn in_(&self, n: i64) -> TermId {
        crate::runtime::values::arena::push_int(self.session_mut(), n)
    }

    /// Machine float atom.
    pub(crate) fn real(&self, x: f64) -> TermId {
        execution::push_number(self.session_mut(), Number::machine(x))
    }

    /// Symbol atom by display name (user symbol, not an operator).
    pub(crate) fn symbol(&self, name: &str) -> TermId {
        crate::runtime::values::arena::push_symbol_name(self.session_mut(), name)
    }

    /// Intern a user symbol name.
    pub(crate) fn intern(&self, name: &str) -> SymbolId {
        self.session_mut().arena.symbols_mut().intern(name)
    }

    /// Resolve a [`SymbolId`] to its display name (user symbol table only).
    pub(crate) fn symbol_resolve(&self, id: SymbolId) -> &str {
        self.session().arena.symbols().resolve(id).unwrap_or("")
    }

    /// Symbol atom from an existing [`SymbolId`].
    pub(crate) fn symbol_id(&self, id: SymbolId) -> TermId {
        let span = athena_ir::TermNode::default_span();
        self.session_mut().arena.push(athena_ir::TermNode::Atom(Atom::Symbol(id)), span)
    }

    /// Explicit collection kind (never a silent `"List"` head).
    pub(crate) fn collection(&self, kind: CollectionKind, items: Vec<TermId>) -> TermId {
        let span = athena_ir::TermNode::default_span();
        self.session_mut().arena.push(athena_ir::TermNode::Collection { kind, elements: items }, span)
    }

    /// Ordered collection convenience.
    pub(crate) fn ordered(&self, items: Vec<TermId>) -> TermId {
        self.collection(CollectionKind::OrderedCollection, items)
    }

    /// Preserve an existing [`ApplicationHead`] when rebuilding.
    pub(crate) fn apply_head(&self, head: ApplicationHead, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_application_head(self.session_mut(), head, args)
    }

    /// Core semantic application.
    pub(crate) fn apply_semantic(&self, op: SemanticOperator, args: Vec<TermId>) -> TermId {
        crate::runtime::values::arena::push_semantic(self.session_mut(), op, args)
    }
}
