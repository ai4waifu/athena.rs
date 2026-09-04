//! 值呈现（供方言 render 与调试；非 legacy `Term` 桥）。

use athena_ir::{Atom, TermNode};
use athena_types::ValueId;

use crate::runtime::{
    session::Session,
    values::arena::{app_args, app_head_name, term_of_value},
};

/// 将 [`ValueId`] 呈现为调试字符串。
pub fn value_debug(session: &Session, value: ValueId) -> String {
    let Some(id) = term_of_value(session, value)
    else {
        return format!("ValueId({})", value.0);
    };
    term_debug(session, id)
}

/// 将 arena 节点呈现为调试字符串。
pub fn term_debug(session: &Session, id: athena_types::TermId) -> String {
    match session.arena.get(id) {
        None => format!("TermId({})", id.0),
        Some(TermNode::Atom(a)) => atom_debug(session, a),
        Some(TermNode::List(items)) => {
            let inner: Vec<_> = items.iter().map(|c| term_debug(session, *c)).collect();
            format!("List[{}]", inner.join(", "))
        }
        Some(TermNode::App { .. }) => {
            let head = app_head_name(session, id).unwrap_or_else(|| "?".into());
            let args = app_args(session, id).unwrap_or_default();
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
