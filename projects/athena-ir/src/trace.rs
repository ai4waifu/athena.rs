//! Core IR 节点 tracing（标记子 [`TermId`] 边；numeric block 由 engine 编排）。

use athena_types::TermId;

use crate::node::TermKind;

/// 标记 [`TermKind`] 持有的子 term 引用。
pub fn trace_term_kind(kind: &TermKind, mark: &mut dyn FnMut(TermId)) {
    match kind {
        TermKind::Atom(_) => {}
        TermKind::List(items) | TermKind::App { args: items, .. } => {
            for child in items {
                mark(*child);
            }
        }
    }
}
