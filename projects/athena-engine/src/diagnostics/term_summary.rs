//! 值呈现（供方言 render 与调试；非 owning AST 桥）。

use athena_ir::{Atom, TermNode};
use athena_types::ValueId;

use crate::runtime::{
    session::Session,
    values::{
        RuntimeValue,
        arena::{application_arguments, application_head_name},
    },
};

/// 将 [`ValueId`] 呈现为调试字符串。
pub fn value_debug(session: &Session, value: ValueId) -> String {
    match session.values.get(value) {
        None => format!("ValueId({})", value.0),
        Some(RuntimeValue::SymbolicTerm(id)) => term_debug(session, *id),
        Some(RuntimeValue::Boolean(true)) => "True".into(),
        Some(RuntimeValue::Boolean(false)) => "False".into(),
        Some(RuntimeValue::Null) => "Null".into(),
        Some(RuntimeValue::Domain(_)) => "DomainResult".into(),
    }
}

/// 将 arena 节点呈现为调试字符串。
pub fn term_debug(session: &Session, id: athena_types::TermId) -> String {
    match session.arena.get(id) {
        None => format!("TermId({})", id.0),
        Some(TermNode::Atom(a)) => atom_debug(session, a),
        Some(TermNode::Collection { elements: items, .. }) => {
            let inner: Vec<_> = items.iter().map(|c| term_debug(session, *c)).collect();
            format!("List[{}]", inner.join(", "))
        }
        Some(TermNode::Application { .. }) => {
            let head = application_head_name(session, id).unwrap_or_else(|| "?".into());
            let args = application_arguments(session, id).unwrap_or_default();
            let inner: Vec<_> = args.iter().map(|c| term_debug(session, *c)).collect();
            format!("{head}[{}]", inner.join(", "))
        }
    }
}

fn atom_debug(session: &Session, atom: &Atom) -> String {
    match atom {
        Atom::Number(n) => n.to_render_string(),
        Atom::String(s) => format!("\"{s}\""),
        Atom::Symbol(sym) => session.arena.symbols().resolve(*sym).unwrap_or("?").to_string(),
        Atom::Boolean(true) => "True".into(),
        Atom::Boolean(false) => "False".into(),
        Atom::Null => "Null".into(),
    }
}
