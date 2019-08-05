//! 模式匹配与替换 — 数据面 `TermId` 结构操作（Session，不经栈式 VM）。
//!
//! 匹配本体为 [`crate::reasoning::trs::TermPattern`]。表面模式项
//! 仅经 [`lower_pattern_term`] 进入，不再作为匹配器分支本体。

use std::collections::HashMap;

use athena_types::{OperatorId, SymbolId, TermId};

use crate::{
    execution::shape::{Shape, push_application_op, term_head_name, term_shape},
    reasoning::trs::TermPattern,
    runtime::{session::Session, values::arena::push_list},
};

/// 结构匹配（无绑定）。
pub(crate) fn pattern_matches(session: &mut Session, expr: TermId, pat: TermId) -> bool {
    pattern_bind(session, expr, pat, &mut HashMap::new())
}

/// 结构匹配并收集绑定（模式项先降为中性 [`TermPattern`]）。
pub(crate) fn pattern_bind(session: &mut Session, expr: TermId, pat: TermId, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    let pattern = lower_pattern_term(session, pat);
    match_term_pattern(session, expr, &pattern, binds)
}

/// 将模式项降为中性 [`TermPattern`]。
pub(crate) fn lower_pattern_term(session: &Session, pat: TermId) -> TermPattern {
    let Some(shape) = term_shape(session, pat)
    else {
        return TermPattern::Exact(pat);
    };
    match shape {
        Shape::Application(op, args) => {
            let name = session.operators.name(op).unwrap_or("").to_string();
            match name.as_str() {
                "Any" => match args.as_slice() {
                    [] => TermPattern::Any,
                    [head_pat] => match term_head_name(session, *head_pat) {
                        Some(head_name) => TermPattern::HeadConstraint { head_name },
                        None => TermPattern::Exact(pat),
                    },
                    _ => TermPattern::Exact(pat),
                },
                "Bind" if args.len() == 2 => {
                    let Some(Shape::Symbol(name_sym)) = term_shape(session, args[0])
                    else {
                        return TermPattern::Exact(pat);
                    };
                    TermPattern::Bind { name: name_sym, inner: Box::new(lower_pattern_term(session, args[1])) }
                }
                _ => TermPattern::StructuralApplication(args.into_iter().map(|a| lower_pattern_term(session, a)).collect()),
            }
        }
        Shape::Collection(items) => TermPattern::Sequence(items.into_iter().map(|i| lower_pattern_term(session, i)).collect()),
        _ => TermPattern::Exact(pat),
    }
}

/// 对中性 [`TermPattern`] 做结构匹配并收集绑定。
pub(crate) fn match_term_pattern(session: &Session, expr: TermId, pattern: &TermPattern, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    match pattern {
        TermPattern::Any => true,
        TermPattern::HeadConstraint { head_name } => head_constraint_holds(session, expr, head_name),
        TermPattern::Bind { name, inner } => {
            if match_term_pattern(session, expr, inner, binds) {
                binds.insert(*name, expr);
                true
            }
            else {
                false
            }
        }
        TermPattern::Exact(literal) => session.arena.structural_eq(expr, *literal),
        TermPattern::Sequence(items) => {
            let Some(Shape::Collection(expr_items)) = term_shape(session, expr)
            else {
                return false;
            };
            zip_match(session, &expr_items, items, binds)
        }
        TermPattern::StructuralApplication(items) => {
            let Some(Shape::Application(_, args)) = term_shape(session, expr)
            else {
                return false;
            };
            zip_match(session, &args, items, binds)
        }
    }
}

fn zip_match(session: &Session, exprs: &[TermId], patterns: &[TermPattern], binds: &mut HashMap<SymbolId, TermId>) -> bool {
    if exprs.len() != patterns.len() {
        return false;
    }
    exprs.iter().zip(patterns.iter()).all(|(e, p)| match_term_pattern(session, *e, p, binds))
}

fn head_constraint_holds(session: &Session, expr: TermId, head_name: &str) -> bool {
    match head_name {
        "Integer" => match term_shape(session, expr) {
            Some(Shape::Number) => match session.arena.get(expr) {
                Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => n.as_exact_integer().is_some() || n.as_integer().is_some(),
                _ => false,
            },
            _ => false,
        },
        "Symbol" => matches!(term_shape(session, expr), Some(Shape::Symbol(_))),
        "List" => matches!(term_shape(session, expr), Some(Shape::Collection(_))),
        "String" => matches!(term_shape(session, expr), Some(Shape::String(_))),
        other => term_head_name(session, expr).is_some_and(|h| h == other),
    }
}

/// `Pattern` 名下的绑定替换：符号原子替换，未命中共享。
pub(crate) fn substitute_binds(session: &mut Session, expr: TermId, binds: &HashMap<SymbolId, TermId>) -> TermId {
    if binds.is_empty() {
        return expr;
    }
    let Some(s) = term_shape(session, expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(symbol) => binds.get(&symbol).copied().unwrap_or(expr),
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::Collection(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = substitute_binds(session, i, binds);
                changed |= r != i;
                out.push(r);
            }
            if changed { push_list(session, out) } else { expr }
        }
        Shape::Application(op, args) => rewrite_app(session, expr, op, args, |session, a| substitute_binds(session, a, binds)),
    }
}

/// 符号替换：`Table` / `CountedLoop` / `Function` 具化。
pub(crate) fn substitute_symbol(session: &mut Session, expr: TermId, symbol: SymbolId, value: TermId) -> TermId {
    let Some(s) = term_shape(session, expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(x) if x == symbol => value,
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::Collection(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = substitute_symbol(session, i, symbol, value);
                changed |= r != i;
                out.push(r);
            }
            if changed { push_list(session, out) } else { expr }
        }
        Shape::Application(op, args) => rewrite_app(session, expr, op, args, |session, a| substitute_symbol(session, a, symbol, value)),
    }
}

/// `Slot` / `#` / `#1` 替换。
pub(crate) fn substitute_slot(session: &mut Session, expr: TermId, value: TermId) -> TermId {
    let Some(s) = term_shape(session, expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(symbol) if is_slot(session, symbol) => value,
        Shape::Application(op, args)
            if session.operators.name(op) == Some("Slot")
                && (args.is_empty()
                    || (args.len() == 1
                        && matches!(
                            session.arena.get(args[0]),
                            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n)))
                                if n.as_exact_integer() == Some(1)
                        ))) =>
        {
            let _ = args;
            value
        }
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::Collection(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = substitute_slot(session, i, value);
                changed |= r != i;
                out.push(r);
            }
            if changed { push_list(session, out) } else { expr }
        }
        Shape::Application(op, args) => rewrite_app(session, expr, op, args, |session, a| substitute_slot(session, a, value)),
    }
}

fn is_slot(session: &Session, s: SymbolId) -> bool {
    matches!(session.arena.symbols().resolve(s), Some("#") | Some("#1"))
}

/// 字面替换（`ReplaceAll`）：结构相等处整体替换。
pub(crate) fn replace_literal(session: &mut Session, expr: TermId, lhs: TermId, rhs: TermId) -> TermId {
    if session.arena.structural_eq(expr, lhs) {
        return rhs;
    }
    let Some(s) = term_shape(session, expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::Collection(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = replace_literal(session, i, lhs, rhs);
                changed |= r != i;
                out.push(r);
            }
            if changed { push_list(session, out) } else { expr }
        }
        Shape::Application(op, args) => rewrite_app(session, expr, op, args, |session, a| replace_literal(session, a, lhs, rhs)),
    }
}

fn rewrite_app(
    session: &mut Session,
    expr: TermId,
    op: OperatorId,
    args: Vec<TermId>,
    mut map: impl FnMut(&mut Session, TermId) -> TermId,
) -> TermId {
    let mut changed = false;
    let mut out = Vec::with_capacity(args.len());
    for a in args {
        let r = map(session, a);
        changed |= r != a;
        out.push(r);
    }
    if changed { push_application_op(session, op, out) } else { expr }
}
