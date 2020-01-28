//! 模式匹配与替换 — 中性 [`TermPattern`]（Living `27`）。
//!
//! 禁止从 `TermId` 解析方言 / 表面 operator 名（`Any` / `Bind` / `Integer` / `List` …）。
//! 需要 `Any` / `Bind` / 类型约束时，由 SXO lowering 直接构造 [`TermPattern`]。

use std::collections::HashMap;

use athena_ir::ApplicationHead;
use athena_types::{SymbolId, TermId};

use crate::{
    execution::shape::{Shape, push_application_head, term_shape},
    reasoning::trs::TermPattern,
    runtime::{session::Session, values::arena::push_list},
};

/// 结构匹配（无绑定）。仅结构降级：`Exact` / `Sequence` / `StructuralApplication`。
pub(crate) fn pattern_matches(session: &mut Session, expr: TermId, pat: TermId) -> bool {
    pattern_bind(session, expr, pat, &mut HashMap::new())
}

/// 结构匹配并收集绑定。
///
/// `pat` 仅作结构项：应用 → 参数位置匹配，集合 → 序列匹配，其余 → `Exact`。
/// 不识别表面名。
pub(crate) fn pattern_bind(session: &mut Session, expr: TermId, pat: TermId, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    let pattern = structural_pattern_from_term(session, pat);
    match_term_pattern(session, expr, &pattern, binds)
}

/// 将 term 降为纯结构 [`TermPattern`]（无字符串语义分支）。
pub(crate) fn structural_pattern_from_term(session: &Session, pat: TermId) -> TermPattern {
    let Some(shape) = term_shape(session, pat)
    else {
        return TermPattern::Exact(pat);
    };
    match shape {
        Shape::Application(op, args) => {
            TermPattern::Application { operator: op, arguments: args.into_iter().map(|a| structural_pattern_from_term(session, a)).collect() }
        }
        Shape::Collection(items) => TermPattern::Sequence(items.into_iter().map(|i| structural_pattern_from_term(session, i)).collect()),
        _ => TermPattern::Exact(pat),
    }
}

/// 对中性 [`TermPattern`] 做结构匹配并收集绑定。
pub fn match_term_pattern(session: &Session, expr: TermId, pattern: &TermPattern, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    crate::reasoning::trs::match_pattern(&session.arena, expr, pattern, binds)
}

/// 绑定替换：符号原子替换，未命中共享。
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
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => expr,
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

/// 符号替换（迭代 / 作用域具化）。
pub(crate) fn substitute_symbol(session: &mut Session, expr: TermId, symbol: SymbolId, value: TermId) -> TermId {
    let Some(s) = term_shape(session, expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(x) if x == symbol => value,
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => expr,
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

/// 字面替换（结构相等处整体替换）。
pub(crate) fn replace_literal(session: &mut Session, expr: TermId, lhs: TermId, rhs: TermId) -> TermId {
    if session.arena.structural_eq(expr, lhs) {
        return rhs;
    }
    let Some(s) = term_shape(session, expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => expr,
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
    head: ApplicationHead,
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
    if changed { push_application_head(session, head, out) } else { expr }
}
