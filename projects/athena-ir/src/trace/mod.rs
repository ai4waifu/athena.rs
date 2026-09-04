//! Core IR 节点 tracing（标记子 [`TermId`] 边；numeric block 由 engine 编排）。

use athena_types::TermId;

use crate::node::TermNode;

/// 标记 [`TermNode`] 持有的子 term 引用。
pub fn trace_term_node(kind: &TermNode, mark: &mut dyn FnMut(TermId)) {
    match kind {
        TermNode::Atom(_) => {}
        TermNode::List(items) | TermNode::Application { arguments: items, .. } => {
            for child in items {
                mark(*child);
            }
        }
    }
}
