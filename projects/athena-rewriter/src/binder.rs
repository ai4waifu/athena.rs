//! 存储层 [`TermPattern`] 绑定器。
//!
//! 仅对 [`TermStore`] 匹配 — 无 `Session`，无方言表层名。

use std::collections::HashMap;

use athena_ir::{Atom, TermNode, TermStore};
use athena_types::{SymbolId, TermId, ValueTypeId};

use crate::pattern::{PatternConstraint, TermPattern};

/// 成功匹配产生的绑定环境。
pub type PatternBindings = HashMap<SymbolId, TermId>;

/// 在 `store` 中将 `pattern` 与 `expr` 匹配，并累积绑定。
///
/// [`TermPattern::Bind`] 保持一致：重复名字必须指向结构上
/// 相等的 term。
pub fn match_pattern(store: &TermStore, expr: TermId, pattern: &TermPattern, binds: &mut PatternBindings) -> bool {
    match pattern {
        TermPattern::Any => true,
        TermPattern::Bind { name, inner } => {
            if !match_pattern(store, expr, inner, binds) {
                return false;
            }
            match binds.get(name) {
                Some(existing) => store.structural_eq(expr, *existing),
                None => {
                    binds.insert(*name, expr);
                    true
                }
            }
        }
        TermPattern::Exact(literal) => store.structural_eq(expr, *literal),
        TermPattern::Sequence(items) => match store.get(expr) {
            Some(TermNode::Collection { elements, .. }) => zip_match(store, elements, items, binds),
            _ => false,
        },
        TermPattern::Application { operator, arguments } => match store.get(expr) {
            Some(TermNode::Application { head, arguments: args }) => head == operator && zip_match(store, args, arguments, binds),
            _ => false,
        },
        TermPattern::StructuralApplication(items) => match store.get(expr) {
            Some(TermNode::Application { arguments: args, .. }) => zip_match(store, args, items, binds),
            _ => false,
        },
        TermPattern::Constrained { pattern, constraint } => {
            constraint_holds(store, expr, constraint) && match_pattern(store, expr, pattern, binds)
        }
    }
}

fn zip_match(store: &TermStore, exprs: &[TermId], patterns: &[TermPattern], binds: &mut PatternBindings) -> bool {
    if exprs.len() != patterns.len() {
        return false;
    }
    exprs.iter().zip(patterns.iter()).all(|(e, p)| match_pattern(store, *e, p, binds))
}

fn constraint_holds(store: &TermStore, expr: TermId, constraint: &PatternConstraint) -> bool {
    match constraint {
        PatternConstraint::Operator(expected) => match store.get(expr) {
            Some(TermNode::Application { head, .. }) => head == expected,
            _ => false,
        },
        PatternConstraint::ValueType(ValueTypeId::ExactInteger) => match store.get(expr) {
            Some(TermNode::Atom(Atom::Number(n))) => n.as_exact_integer().is_some() || n.as_integer().is_some(),
            _ => false,
        },
        PatternConstraint::ValueType(ValueTypeId::Symbol) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Symbol(_))))
        }
        PatternConstraint::ValueType(ValueTypeId::String) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::String(_))))
        }
        PatternConstraint::ValueType(ValueTypeId::Boolean) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Boolean(_))))
        }
        PatternConstraint::ValueType(ValueTypeId::Null) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Null)))
        }
        PatternConstraint::ValueType(ValueTypeId::Numeric) => {
            matches!(store.get(expr), Some(TermNode::Atom(Atom::Number(_))))
        }
        PatternConstraint::CollectionKind(kind) => match store.get(expr) {
            Some(TermNode::Collection { kind: k, .. }) => k == kind,
            _ => false,
        },
        PatternConstraint::Domain(_) | PatternConstraint::Predicate(_) => false,
    }
}

/// 将 `binds` 应用到模板 term，并把重建节点 hash-cons 进 `store`。
///
/// `binds` 中出现的符号原子会被替换。未绑定符号与其他原子共享。
pub fn substitute(store: &mut TermStore, template: TermId, binds: &PatternBindings) -> TermId {
    if binds.is_empty() {
        return template;
    }
    let span = store.span(template).unwrap_or_default();
    match store.get(template) {
        None => template,
        Some(TermNode::Atom(Atom::Symbol(sym))) => binds.get(sym).copied().unwrap_or(template),
        Some(TermNode::Atom(_)) => template,
        Some(TermNode::Collection { kind, elements }) => {
            let kind = *kind;
            let elements = elements.clone();
            let out: Vec<TermId> = elements.iter().map(|e| substitute(store, *e, binds)).collect();
            if out == elements { template } else { store.push(TermNode::Collection { kind, elements: out }, span) }
        }
        Some(TermNode::Application { head, arguments }) => {
            let head = *head;
            let arguments = arguments.clone();
            let out: Vec<TermId> = arguments.iter().map(|a| substitute(store, *a, binds)).collect();
            if out == arguments { template } else { store.push(TermNode::Application { head, arguments: out }, span) }
        }
    }
}
