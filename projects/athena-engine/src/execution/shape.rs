//! Cheap structural snapshot of a `TermId` (no numeric payload copy).

use athena_ir::{Atom, TermNode};
use athena_types::{OperatorId, SymbolId, TermId};

use crate::runtime::session::Session;

/// Cheap structural snapshot (numbers are tagged `Number` without copying payload).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shape {
    Number,
    String(String),
    Symbol(SymbolId),
    Bool(bool),
    Null,
    Collection(Vec<TermId>),
    Application(OperatorId, Vec<TermId>),
}

/// Structural snapshot from a Session arena (no VM).
pub(crate) fn term_shape(session: &Session, id: TermId) -> Option<Shape> {
    match session.arena.get(id)? {
        TermNode::Atom(Atom::Number(_)) => Some(Shape::Number),
        TermNode::Atom(Atom::String(s)) => Some(Shape::String(s.clone())),
        TermNode::Atom(Atom::Symbol(s)) => Some(Shape::Symbol(*s)),
        TermNode::Atom(Atom::Boolean(b)) => Some(Shape::Bool(*b)),
        TermNode::Atom(Atom::Null) => Some(Shape::Null),
        TermNode::Collection { elements: items, .. } => Some(Shape::Collection(items.clone())),
        TermNode::Application { head: op, arguments: args } => Some(Shape::Application(*op, args.clone())),
    }
}

/// Head display name for atoms / apps / lists (Session path).
pub(crate) fn term_head_name(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id)? {
        TermNode::Application { head: op, .. } => session.operators.name(*op).map(str::to_string),
        TermNode::Collection { .. } => Some("OrderedCollection".into()),
        TermNode::Atom(Atom::Symbol(symbol)) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

/// Rebuild an operator application in the Session arena.
pub(crate) fn push_application_op(session: &mut Session, op: OperatorId, args: Vec<TermId>) -> TermId {
    let span = TermNode::default_span();
    session.arena.push(TermNode::Application { head: op, arguments: args }, span)
}
