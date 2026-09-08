//! `Apply` / `ApplyHead` 应用形态。

use athena_ir::SemanticOperator;
use athena_types::{Result, TermId};

use super::{re_eval_term, rebuild_application, try_apply_callable};
use crate::{execution::push_semantic, runtime::session::Session};

/// `Apply[head, list]` — 列表实参展开后重建应用并再求值。
pub(crate) fn evaluate_apply_terms(session: &mut Session, head: TermId, second: TermId) -> Result<TermId> {
    let items = match session.arena.get(second) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return Ok(push_semantic(session, SemanticOperator::Apply, vec![head, second])),
    };
    let app = rebuild_application(session, head, items);
    re_eval_term(session, app)
}

/// `ApplyHead[head, args…]` — `Function` 绑定（单参或多参 binder 列表）或 typed 残差。
pub(crate) fn evaluate_apply_head_terms(session: &mut Session, head: TermId, call_args: Vec<TermId>) -> Result<TermId> {
    if let Some(term) = try_apply_callable(session, head, &call_args)? {
        return Ok(term);
    }
    // 禁止裸符号经显示名 intern 成扩展算子；保留 typed `ApplyHead` 残差。
    let mut wrapped = Vec::with_capacity(call_args.len() + 1);
    wrapped.push(head);
    wrapped.extend(call_args);
    Ok(push_semantic(session, SemanticOperator::ApplyHead, wrapped))
}
