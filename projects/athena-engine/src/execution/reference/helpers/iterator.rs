//! 迭代器展开与 `Sum` / `Product` 折叠。

use athena_ir::SemanticOperator;
use athena_types::{Result, TermId};

use super::{
    expand_iterator_session, fold_plus_symbolic, fold_times_symbolic, re_eval_term,
};
use crate::{execution::push_semantic, runtime::session::Session};

/// 展开 iterator，对 body 逐值替换并再求值。
pub(crate) fn table_values_session(session: &mut Session, body: TermId, iter: TermId) -> Result<Option<Vec<TermId>>> {
    let Some((var, values)) = expand_iterator_session(session, iter) else {
        return Ok(None);
    };
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let instantiated = match var {
            Some(sym) => crate::execution::builtins::patterns::substitute_symbol(session, body, sym, value),
            None => body,
        };
        out.push(re_eval_term(session, instantiated)?);
    }
    Ok(Some(out))
}

/// `Sum[body, iterator]` 或不可展开时的残差。
pub(crate) fn evaluate_sum_iterator_terms(session: &mut Session, body: TermId, iter: TermId) -> Result<TermId> {
    match table_values_session(session, body, iter)? {
        Some(values) if values.is_empty() => Ok(session.builder().int(0, Default::default())),
        Some(values) => Ok(fold_plus_symbolic(session, values)),
        None => Ok(push_semantic(session, SemanticOperator::Sum, vec![body, iter])),
    }
}

/// `Product[body, iterator]` 或不可展开时的残差。
pub(crate) fn evaluate_product_iterator_terms(session: &mut Session, body: TermId, iter: TermId) -> Result<TermId> {
    match table_values_session(session, body, iter)? {
        Some(values) if values.is_empty() => Ok(session.builder().int(1, Default::default())),
        Some(values) => Ok(fold_times_symbolic(session, values)),
        None => Ok(push_semantic(session, SemanticOperator::Product, vec![body, iter])),
    }
}

/// 非迭代器 `Product[args…]` — 当前仅保残差（无列表积折叠）。
pub(crate) fn evaluate_product_terms(session: &mut Session, terms: Vec<TermId>) -> Result<TermId> {
    Ok(push_semantic(session, SemanticOperator::Product, terms))
}
