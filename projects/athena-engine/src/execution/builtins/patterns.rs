//! 模式匹配与替换 — 中性 [`TermPattern`]（Living `27`）。
//!
//! 禁止从 `TermId` 解析方言 / 表面 operator 名（`Any` / `Bind` / `Integer` / `List` …）。
//! 需要 `Any` / `Bind` / 类型约束时，由 SXO lowering 直接构造 [`TermPattern`]。

use std::collections::HashMap;

use athena_ir::ApplicationHead;
use athena_types::{SymbolId, TermId, ValueTypeId};

use crate::{
    execution::shape::{Shape, push_application_head, term_shape},
    reasoning::trs::{PatternConstraint, TermPattern},
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
        Shape::Application(op, args) => TermPattern::Application {
            operator: op,
            arguments: args.into_iter().map(|a| structural_pattern_from_term(session, a)).collect(),
        },
        Shape::Collection(items) => TermPattern::Sequence(items.into_iter().map(|i| structural_pattern_from_term(session, i)).collect()),
        _ => TermPattern::Exact(pat),
    }
}

/// 对中性 [`TermPattern`] 做结构匹配并收集绑定。
pub fn match_term_pattern(session: &Session, expr: TermId, pattern: &TermPattern, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    match pattern {
        TermPattern::Any => true,
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
        TermPattern::Application { operator, arguments } => {
            let Some(Shape::Application(op, args)) = term_shape(session, expr)
            else {
                return false;
            };
            op == *operator && zip_match(session, &args, arguments, binds)
        }
        TermPattern::StructuralApplication(items) => {
            let Some(Shape::Application(_, args)) = term_shape(session, expr)
            else {
                return false;
            };
            zip_match(session, &args, items, binds)
        }
        TermPattern::Constrained { pattern, constraint } => {
            constraint_holds(session, expr, constraint) && match_term_pattern(session, expr, pattern, binds)
        }
    }
}

fn zip_match(session: &Session, exprs: &[TermId], patterns: &[TermPattern], binds: &mut HashMap<SymbolId, TermId>) -> bool {
    if exprs.len() != patterns.len() {
        return false;
    }
    exprs.iter().zip(patterns.iter()).all(|(e, p)| match_term_pattern(session, *e, p, binds))
}

fn constraint_holds(session: &Session, expr: TermId, constraint: &PatternConstraint) -> bool {
    match constraint {
        PatternConstraint::Operator(expected) => match term_shape(session, expr) {
            Some(Shape::Application(op, _)) => op == *expected,
            _ => false,
        },
        PatternConstraint::ValueType(ValueTypeId::ExactInteger) => match session.arena.get(expr) {
            Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => n.as_exact_integer().is_some() || n.as_integer().is_some(),
            _ => false,
        },
        PatternConstraint::ValueType(ValueTypeId::Symbol) => matches!(term_shape(session, expr), Some(Shape::Symbol(_))),
        PatternConstraint::ValueType(ValueTypeId::String) => matches!(term_shape(session, expr), Some(Shape::String(_))),
        PatternConstraint::ValueType(ValueTypeId::Boolean) => matches!(term_shape(session, expr), Some(Shape::Bool(_))),
        PatternConstraint::ValueType(ValueTypeId::Null) => matches!(term_shape(session, expr), Some(Shape::Null)),
        PatternConstraint::ValueType(ValueTypeId::Numeric) => matches!(term_shape(session, expr), Some(Shape::Number)),
        PatternConstraint::CollectionKind(kind) => match session.arena.get(expr) {
            Some(athena_ir::TermNode::Collection { kind: k, .. }) => k == kind,
            _ => false,
        },
        PatternConstraint::Domain(_) | PatternConstraint::Predicate(_) => false,
    }
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

/// 符号替换（迭代 / 作用域具化）。
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
