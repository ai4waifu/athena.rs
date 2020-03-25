//! `TermId` 的廉价结构快照（不复制数值载荷）。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::{SymbolId, TermId};

use crate::runtime::session::Session;

/// 廉价结构快照（数字仅打 `Number` 标签，不复制载荷）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Shape {
    Number,
    String(String),
    Symbol(SymbolId),
    Bool(bool),
    Null,
    Constant(athena_ir::MathematicalConstant),
    Collection(Vec<TermId>),
    Application(ApplicationHead, Vec<TermId>),
}

/// 从 Session arena 取结构快照（无 VM）。
pub(crate) fn term_shape(session: &Session, id: TermId) -> Option<Shape> {
    match session.arena.get(id)? {
        TermNode::Atom(Atom::Number(_)) => Some(Shape::Number),
        TermNode::Atom(Atom::String(s)) => Some(Shape::String(s.clone())),
        TermNode::Atom(Atom::Symbol(s)) => Some(Shape::Symbol(*s)),
        TermNode::Atom(Atom::Boolean(b)) => Some(Shape::Bool(*b)),
        TermNode::Atom(Atom::Null) => Some(Shape::Null),
        TermNode::Atom(Atom::Constant(c)) => Some(Shape::Constant(*c)),
        TermNode::Collection { elements: items, .. } => Some(Shape::Collection(items.clone())),
        TermNode::Application { head, arguments: args } => Some(Shape::Application(*head, args.clone())),
    }
}

/// 调试 / 诊断用的头标签（Session 路径）。
///
/// **不得用于语义分派**。优先使用 [`crate::runtime::values::arena::application_head`]。
pub(crate) fn debug_term_head_label(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id)? {
        TermNode::Application { head, .. } => match *head {
            ApplicationHead::Semantic(op) => Some(op.debug_label().to_string()),
            ApplicationHead::Extension(op) => session.extensions.display_name(op).map(str::to_string),
        },
        TermNode::Collection { kind, .. } => Some(kind.debug_label().to_string()),
        TermNode::Atom(Atom::Symbol(symbol)) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        TermNode::Atom(Atom::Constant(c)) => Some(c.debug_label().to_string()),
        _ => None,
    }
}

/// 在 Session arena 中重建语义算子应用。
pub(crate) fn push_application_semantic(session: &mut Session, op: SemanticOperator, args: Vec<TermId>) -> TermId {
    let span = TermNode::default_span();
    session.arena.push(TermNode::Application { head: ApplicationHead::Semantic(op), arguments: args }, span)
}

/// 从 [`ApplicationHead`] 重建应用。
pub(crate) fn push_application_head(session: &mut Session, head: ApplicationHead, args: Vec<TermId>) -> TermId {
    let span = TermNode::default_span();
    session.arena.push(TermNode::Application { head, arguments: args }, span)
}
