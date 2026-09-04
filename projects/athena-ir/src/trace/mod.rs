//! Core IR 节点 tracing（标记子 [`ExprId`] 边；numeric block 由 engine 编排）。

use athena_types::ExprId;

use crate::node::ExprNode;

/// 标记 [`ExprNode`] 持有的子 term 引用。
pub fn trace_expr_node(kind: &ExprNode, mark: &mut dyn FnMut(ExprId)) {
    match kind {
        ExprNode::Atom(_) => {}
        ExprNode::List(items) | ExprNode::App { args: items, .. } => {
            for child in items {
                mark(*child);
            }
        }
    }
}
