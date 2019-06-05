//! 模式匹配与替换 — 数据面 `TermId` 结构操作。
//!
//! 匹配本体为 [`crate::reasoning::trs::TermPattern`]。方言 `Blank` / `Pattern`
//! 仅经 [`lower_from_dialect_term`] 进入，不再作为匹配器分支本体。

use std::collections::HashMap;

use athena_types::{SymbolId, TermId};

use crate::{
    execution::vm::{Shape, Vm},
    reasoning::trs::TermPattern,
};

/// MatchQ 式匹配（无绑定）。
pub(crate) fn pattern_matches(vm: &mut Vm<'_>, expr: TermId, pat: TermId) -> bool {
    pattern_bind(vm, expr, pat, &mut HashMap::new())
}

/// 结构匹配并收集绑定（方言模式项先降为中性 [`TermPattern`]）。
pub(crate) fn pattern_bind(vm: &mut Vm<'_>, expr: TermId, pat: TermId, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    let pattern = lower_from_dialect_term(vm, pat);
    match_term_pattern(vm, expr, &pattern, binds)
}

/// 将方言表面模式项降为中性 [`TermPattern`]。
pub(crate) fn lower_from_dialect_term(vm: &mut Vm<'_>, pat: TermId) -> TermPattern {
    let Some(shape) = vm.shape(pat)
    else {
        return TermPattern::Exact(pat);
    };
    match shape {
        Shape::Application(op, args) => {
            let name = vm.session.operators.name(op).unwrap_or("").to_string();
            match name.as_str() {
                "Blank" => match args.as_slice() {
                    [] => TermPattern::Any,
                    [head_pat] => match vm.head_name(*head_pat) {
                        Some(head_name) => TermPattern::HeadConstraint { head_name },
                        None => TermPattern::Exact(pat),
                    },
                    _ => TermPattern::Exact(pat),
                },
                "Pattern" if args.len() == 2 => {
                    let Some(Shape::Symbol(name_sym)) = vm.shape(args[0])
                    else {
                        return TermPattern::Exact(pat);
                    };
                    TermPattern::Bind { name: name_sym, inner: Box::new(lower_from_dialect_term(vm, args[1])) }
                }
                _ => TermPattern::StructuralApplication(args.into_iter().map(|a| lower_from_dialect_term(vm, a)).collect()),
            }
        }
        Shape::List(items) => TermPattern::Sequence(items.into_iter().map(|i| lower_from_dialect_term(vm, i)).collect()),
        _ => TermPattern::Exact(pat),
    }
}

/// 对中性 [`TermPattern`] 做结构匹配并收集绑定。
pub(crate) fn match_term_pattern(vm: &mut Vm<'_>, expr: TermId, pattern: &TermPattern, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    match pattern {
        TermPattern::Any => true,
        TermPattern::HeadConstraint { head_name } => head_constraint_holds(vm, expr, head_name),
        TermPattern::Bind { name, inner } => {
            if match_term_pattern(vm, expr, inner, binds) {
                binds.insert(*name, expr);
                true
            }
            else {
                false
            }
        }
        TermPattern::Exact(literal) => vm.session.arena.structural_eq(expr, *literal),
        TermPattern::Sequence(items) => {
            let Some(Shape::List(expr_items)) = vm.shape(expr)
            else {
                return false;
            };
            zip_match(vm, &expr_items, items, binds)
        }
        TermPattern::StructuralApplication(items) => {
            let Some(Shape::Application(_, args)) = vm.shape(expr)
            else {
                return false;
            };
            zip_match(vm, &args, items, binds)
        }
    }
}

fn zip_match(vm: &mut Vm<'_>, exprs: &[TermId], patterns: &[TermPattern], binds: &mut HashMap<SymbolId, TermId>) -> bool {
    if exprs.len() != patterns.len() {
        return false;
    }
    exprs.iter().zip(patterns.iter()).all(|(e, p)| match_term_pattern(vm, *e, p, binds))
}

fn head_constraint_holds(vm: &mut Vm<'_>, expr: TermId, head_name: &str) -> bool {
    match head_name {
        "Integer" => match vm.shape(expr) {
            Some(Shape::Number) => match vm.session.arena.get(expr) {
                Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => n.as_exact_integer().is_some() || n.as_integer().is_some(),
                _ => false,
            },
            _ => false,
        },
        "Symbol" => matches!(vm.shape(expr), Some(Shape::Symbol(_))),
        "List" => matches!(vm.shape(expr), Some(Shape::List(_))),
        "String" => matches!(vm.shape(expr), Some(Shape::String(_))),
        other => vm.head_name(expr).is_some_and(|h| h == other),
    }
}

/// head 约束判定（经中性 `HeadConstraint`）。
pub(crate) fn expr_has_head(vm: &mut Vm<'_>, expr: TermId, head_pat: TermId) -> bool {
    let Some(head_name) = vm.head_name(head_pat)
    else {
        return false;
    };
    match_term_pattern(vm, expr, &TermPattern::HeadConstraint { head_name }, &mut HashMap::new())
}

/// `Pattern` 名下的绑定替换（legacy `substitute_pattern_binds`）：符号原子替换，未命中共享。
pub(crate) fn substitute_binds(vm: &mut Vm<'_>, expr: TermId, binds: &HashMap<SymbolId, TermId>) -> TermId {
    if binds.is_empty() {
        return expr;
    }
    let Some(s) = vm.shape(expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(symbol) => binds.get(&symbol).copied().unwrap_or(expr),
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::List(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = substitute_binds(vm, i, binds);
                changed |= r != i;
                out.push(r);
            }
            if changed { vm.push_list(out) } else { expr }
        }
        Shape::Application(op, args) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = substitute_binds(vm, a, binds);
                changed |= r != a;
                out.push(r);
            }
            if changed { vm.rebuild_application_operator(op, out) } else { expr }
        }
    }
}

/// 符号替换（legacy `substitute_symbol`）：Table / For / Function 具化。
pub(crate) fn substitute_symbol(vm: &mut Vm<'_>, expr: TermId, symbol: SymbolId, value: TermId) -> TermId {
    let Some(s) = vm.shape(expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(x) if x == symbol => value,
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::List(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = substitute_symbol(vm, i, symbol, value);
                changed |= r != i;
                out.push(r);
            }
            if changed { vm.push_list(out) } else { expr }
        }
        Shape::Application(op, args) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = substitute_symbol(vm, a, symbol, value);
                changed |= r != a;
                out.push(r);
            }
            if changed { vm.rebuild_application_operator(op, out) } else { expr }
        }
    }
}

/// `Slot` / `#` / `#1` 替换（legacy `substitute_slot`）。
pub(crate) fn substitute_slot(vm: &mut Vm<'_>, expr: TermId, value: TermId) -> TermId {
    let Some(s) = vm.shape(expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(symbol) if is_slot(vm, symbol) => value,
        Shape::Application(op, args)
            if vm.session.operators.name(op) == Some("Slot")
                && (args.is_empty()
                    || (args.len() == 1
                        && matches!(vm.session.arena.get(args[0]), Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer() == Some(1)))) =>
        {
            let _ = args;
            value
        }
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::List(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = substitute_slot(vm, i, value);
                changed |= r != i;
                out.push(r);
            }
            if changed { vm.push_list(out) } else { expr }
        }
        Shape::Application(op, args) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = substitute_slot(vm, a, value);
                changed |= r != a;
                out.push(r);
            }
            if changed { vm.rebuild_application_operator(op, out) } else { expr }
        }
    }
}

fn is_slot(vm: &Vm<'_>, s: SymbolId) -> bool {
    matches!(vm.session.arena.symbols().resolve(s), Some("#") | Some("#1"))
}

/// 字面替换（legacy `replace_literal`，`ReplaceAll` 用）：结构相等处整体替换。
pub(crate) fn replace_literal(vm: &mut Vm<'_>, expr: TermId, lhs: TermId, rhs: TermId) -> TermId {
    if vm.session.arena.structural_eq(expr, lhs) {
        return rhs;
    }
    let Some(s) = vm.shape(expr)
    else {
        return expr;
    };
    match s {
        Shape::Symbol(_) | Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::List(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = replace_literal(vm, i, lhs, rhs);
                changed |= r != i;
                out.push(r);
            }
            if changed { vm.push_list(out) } else { expr }
        }
        Shape::Application(op, args) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = replace_literal(vm, a, lhs, rhs);
                changed |= r != a;
                out.push(r);
            }
            if changed { vm.rebuild_application_operator(op, out) } else { expr }
        }
    }
}
