//! Cheap structural snapshot of a `TermId` (no numeric payload copy).

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::{SymbolId, TermId};

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
    Application(ApplicationHead, Vec<TermId>),
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
        TermNode::Application { head, arguments: args } => Some(Shape::Application(*head, args.clone())),
    }
}

/// Head display name for atoms / apps / lists (Session path).
pub(crate) fn term_head_name(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id)? {
        TermNode::Application { head, .. } => match *head {
            ApplicationHead::Semantic(op) => Some(op.debug_label().to_string()),
            ApplicationHead::Extension(op) => session.operators.name(op).map(str::to_string),
        },
        TermNode::Collection { .. } => Some("OrderedCollection".into()),
        TermNode::Atom(Atom::Symbol(symbol)) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

/// Rebuild a semantic operator application in the Session arena.
pub(crate) fn push_application_semantic(session: &mut Session, op: SemanticOperator, args: Vec<TermId>) -> TermId {
    let span = TermNode::default_span();
    session.arena.push(
        TermNode::Application {
            head: ApplicationHead::Semantic(op),
            arguments: args,
        },
        span,
    )
}

/// Rebuild an application from an [`ApplicationHead`].
pub(crate) fn push_application_head(session: &mut Session, head: ApplicationHead, args: Vec<TermId>) -> TermId {
    let span = TermNode::default_span();
    session.arena.push(TermNode::Application { head, arguments: args }, span)
}
