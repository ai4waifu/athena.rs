//! `Map` 列表映射（零元算子头 / `Function[var, body]`）。

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_types::{Result, TermId};

use super::{diag, re_eval_term};
use crate::{
    execution::push_semantic,
    runtime::{session::Session, values::arena::push_list},
};

fn map_func_supported(session: &Session, func: TermId) -> bool {
    match session.arena.get(func) {
        Some(athena_ir::TermNode::Application {
            head: ApplicationHead::Semantic(SemanticOperator::Function),
            arguments,
        }) if arguments.len() == 2 => true,
        Some(athena_ir::TermNode::Application {
            head: ApplicationHead::Semantic(_) | ApplicationHead::Extension(_),
            arguments,
        }) if arguments.is_empty() => true,
        _ => false,
    }
}

fn map_apply_one(session: &mut Session, func: TermId, item: TermId) -> Result<TermId> {
    if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(func) {
        if arguments.is_empty() {
            let mapped = match *head {
                ApplicationHead::Semantic(op) => push_semantic(session, op, vec![item]),
                ApplicationHead::Extension(id) => {
                    let mut b = athena_ir::TermBuilder::new(&mut session.arena);
                    b.application_extension_id(id, vec![item], athena_ir::TermNode::default_span())
                }
            };
            return re_eval_term(session, mapped);
        }
        if matches!(*head, ApplicationHead::Semantic(SemanticOperator::Function)) {
            let arguments = arguments.clone();
            if let [var, body] = arguments.as_slice() {
                if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(sym))) = session.arena.get(*var) {
                    let instantiated =
                        crate::execution::builtins::patterns::substitute_symbol(session, *body, *sym, item);
                    return re_eval_term(session, instantiated);
                }
            }
        }
    }
    // 禁止 `symbol_name` → `extensions.intern`：裸符号头须由编译期 / 方言 lowering
    // 落成 `ApplicationHead::Extension` 或封闭 `SemanticOperator`。
    Err(diag("map_func_unsupported"))
}

/// `Map[func, list]` — 支持列表则逐元应用，否则残差。
pub(crate) fn evaluate_map_terms(session: &mut Session, func: TermId, list: TermId) -> Result<TermId> {
    let items = match session.arena.get(list) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return Ok(push_semantic(session, SemanticOperator::Map, vec![func, list])),
    };
    if !map_func_supported(session, func) {
        return Ok(push_semantic(session, SemanticOperator::Map, vec![func, list]));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(map_apply_one(session, func, item)?);
    }
    Ok(push_list(session, out))
}
