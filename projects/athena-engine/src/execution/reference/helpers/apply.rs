//! `Apply` / `ApplyHead` 应用形态。

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_types::{Result, TermId};

use super::{rebuild_application, re_eval_term};
use crate::{
    execution::push_semantic,
    runtime::session::Session,
};

/// `Apply[head, list]` — 列表实参展开后重建应用并再求值。
pub(crate) fn evaluate_apply_terms(session: &mut Session, head: TermId, second: TermId) -> Result<TermId> {
    let items = match session.arena.get(second) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return Ok(push_semantic(session, SemanticOperator::Apply, vec![head, second])),
    };
    let app = rebuild_application(session, head, items);
    re_eval_term(session, app)
}

/// `ApplyHead[head, args…]` — `Function[var, body]` 绑定或 typed 残差。
pub(crate) fn evaluate_apply_head_terms(session: &mut Session, head: TermId, call_args: Vec<TermId>) -> Result<TermId> {
    // `Function[var, body][arg…]` → 替换并重新求值。
    // 纯 `Function[body]` 需要方言 lowering 的 `AnonymousArgument`（不是字符串 Slot）。
    if let Some(athena_ir::TermNode::Application { head: op, arguments }) = session.arena.get(head) {
        if matches!(*op, ApplicationHead::Semantic(SemanticOperator::Function)) && call_args.len() == 1 {
            let arguments = arguments.clone();
            if let [var, body] = arguments.as_slice() {
                if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(sym))) = session.arena.get(*var) {
                    let sym = *sym;
                    let instantiated =
                        crate::execution::builtins::patterns::substitute_symbol(session, *body, sym, call_args[0]);
                    return re_eval_term(session, instantiated);
                }
            }
        }
    }
    // 禁止裸符号经显示名 intern 成扩展算子；保留 typed `ApplyHead` 残差。
    let mut wrapped = Vec::with_capacity(call_args.len() + 1);
    wrapped.push(head);
    wrapped.extend(call_args);
    Ok(push_semantic(session, SemanticOperator::ApplyHead, wrapped))
}
