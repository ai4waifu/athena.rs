//! `Map` / `MapIndexed` 列表映射（零元算子头 / `Function` 绑定）。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_types::{Result, SymbolId, TermId};

use super::{diag, re_eval_term};
use crate::{
    execution::push_semantic,
    runtime::{session::Session, values::arena::push_list},
};

/// `Function[var, body]` 或 `Function[{v₁,…}, body]` 的 binder 符号列。
fn function_binders(session: &Session, func: TermId) -> Option<Vec<SymbolId>> {
    let TermNode::Application {
        head: ApplicationHead::Semantic(SemanticOperator::Function),
        arguments,
    } = session.arena.get(func)?
    else {
        return None;
    };
    if arguments.len() != 2 {
        return None;
    }
    match session.arena.get(arguments[0])? {
        TermNode::Atom(Atom::Symbol(sym)) => Some(vec![*sym]),
        TermNode::Collection { elements, .. } => {
            let mut out = Vec::with_capacity(elements.len());
            for id in elements {
                match session.arena.get(*id)? {
                    TermNode::Atom(Atom::Symbol(sym)) => out.push(*sym),
                    _ => return None,
                }
            }
            Some(out)
        }
        _ => None,
    }
}

fn function_body(session: &Session, func: TermId) -> Option<TermId> {
    let TermNode::Application {
        head: ApplicationHead::Semantic(SemanticOperator::Function),
        arguments,
    } = session.arena.get(func)?
    else {
        return None;
    };
    (arguments.len() == 2).then_some(arguments[1])
}

fn map_func_supported(session: &Session, func: TermId, arity: usize) -> bool {
    if let Some(binders) = function_binders(session, func) {
        return binders.len() == arity;
    }
    match session.arena.get(func) {
        Some(TermNode::Application {
            head: ApplicationHead::Semantic(_) | ApplicationHead::Extension(_),
            arguments,
        }) if arguments.is_empty() && arity == 1 => true,
        _ => false,
    }
}

/// 将 `Function` 或零元头应用到 `call_args`；arity 不匹配时返回 `None`。
pub(crate) fn try_apply_callable(session: &mut Session, func: TermId, call_args: &[TermId]) -> Result<Option<TermId>> {
    if let Some(binders) = function_binders(session, func) {
        if binders.len() != call_args.len() {
            return Ok(None);
        }
        let Some(mut body) = function_body(session, func)
        else {
            return Ok(None);
        };
        for (sym, value) in binders.into_iter().zip(call_args.iter().copied()) {
            body = crate::execution::builtins::patterns::substitute_symbol(session, body, sym, value);
        }
        return Ok(Some(re_eval_term(session, body)?));
    }
    if call_args.len() == 1 {
        if let Some(TermNode::Application { head, arguments }) = session.arena.get(func) {
            if arguments.is_empty() {
                let item = call_args[0];
                let mapped = match *head {
                    ApplicationHead::Semantic(op) => push_semantic(session, op, vec![item]),
                    ApplicationHead::Extension(id) => {
                        let mut b = athena_ir::TermBuilder::new(&mut session.arena);
                        b.application_extension_id(id, vec![item], TermNode::default_span())
                    }
                };
                return Ok(Some(re_eval_term(session, mapped)?));
            }
        }
    }
    Ok(None)
}

fn map_apply_one(session: &mut Session, func: TermId, item: TermId) -> Result<TermId> {
    match try_apply_callable(session, func, &[item])? {
        Some(term) => Ok(term),
        None => Err(diag("map_func_unsupported")),
    }
}

/// `Map[func, list]` — 支持列表则逐元应用，否则残差。
pub(crate) fn evaluate_map_terms(session: &mut Session, func: TermId, list: TermId) -> Result<TermId> {
    let items = match session.arena.get(list) {
        Some(TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return Ok(push_semantic(session, SemanticOperator::Map, vec![func, list])),
    };
    if !map_func_supported(session, func, 1) {
        return Ok(push_semantic(session, SemanticOperator::Map, vec![func, list]));
    }
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        out.push(map_apply_one(session, func, item)?);
    }
    Ok(push_list(session, out))
}

/// `MapIndexed[func, list]` — `func[elem, {i}]`，`i` 从 1 起。
pub(crate) fn evaluate_map_indexed_terms(session: &mut Session, func: TermId, list: TermId) -> Result<TermId> {
    let items = match session.arena.get(list) {
        Some(TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return Ok(push_semantic(session, SemanticOperator::MapIndexed, vec![func, list])),
    };
    if !map_func_supported(session, func, 2) {
        return Ok(push_semantic(session, SemanticOperator::MapIndexed, vec![func, list]));
    }
    let mut out = Vec::with_capacity(items.len());
    for (i, item) in items.into_iter().enumerate() {
        let index = session.builder().int((i as i64) + 1, Default::default());
        let index_list = push_list(session, vec![index]);
        match try_apply_callable(session, func, &[item, index_list])? {
            Some(term) => out.push(term),
            None => return Ok(push_semantic(session, SemanticOperator::MapIndexed, vec![func, list])),
        }
    }
    Ok(push_list(session, out))
}
