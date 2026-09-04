//! 模式匹配与替换 — 数据面 `TermId` 结构操作（legacy `pattern_*` / `substitute_*` 语义）。
//!
//! 只重建被改动的路径，未改动子树按 arena 地址共享。

use std::collections::HashMap;

use athena_types::{SymbolId, TermId};

use crate::execution::vm::{Shape, Vm};

/// MatchQ 式匹配（无绑定）。
pub(crate) fn pattern_matches(vm: &mut Vm<'_>, expr: TermId, pat: TermId) -> bool {
    pattern_bind(vm, expr, pat, &mut HashMap::new())
}

/// 结构匹配并收集 `Pattern[name, p]` 绑定。
pub(crate) fn pattern_bind(vm: &mut Vm<'_>, expr: TermId, pat: TermId, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    let Some(ps) = vm.shape(pat)
    else {
        return false;
    };
    match ps {
        Shape::Application(op, args) => {
            let name = vm.session.operators.name(op).unwrap_or("").to_string();
            match name.as_str() {
                "Blank" => match args.as_slice() {
                    [] => true,
                    [head_pat] => expr_has_head(vm, expr, *head_pat),
                    _ => false,
                },
                "Pattern" if args.len() == 2 => {
                    let Some(Shape::Symbol(name_sym)) = vm.shape(args[0])
                    else {
                        return false;
                    };
                    if pattern_bind(vm, expr, args[1], binds) {
                        binds.insert(name_sym, expr);
                        true
                    }
                    else {
                        false
                    }
                }
                _ => structural_bind(vm, expr, pat, binds),
            }
        }
        Shape::List(_) => structural_bind(vm, expr, pat, binds),
        _ => vm.session.arena.structural_eq(expr, pat),
    }
}

/// List↔List / App↔App 结构匹配；App 的 head 按算子名一致判定
/// （legacy head term 匹配对符号 head 等价于名相等）。
fn structural_bind(vm: &mut Vm<'_>, expr: TermId, pat: TermId, binds: &mut HashMap<SymbolId, TermId>) -> bool {
    let Some(p) = vm.shape(pat)
    else {
        return vm.session.arena.structural_eq(expr, pat);
    };
    let (pat_app, pat_items) = match p {
        Shape::List(v) => (false, v),
        Shape::Application(op, v) => (true, v),
        _ => return vm.session.arena.structural_eq(expr, pat),
    };
    let Some(e) = vm.shape(expr)
    else {
        return false;
    };
    let (expr_app, expr_items) = match e {
        Shape::List(v) => (false, v),
        Shape::Application(_, v) => (true, v),
        _ => return false,
    };
    if pat_app != expr_app {
        return false;
    }
    pat_items.len() == expr_items.len()
        && pat_items.iter().zip(expr_items.iter()).all(|(p2, e2)| pattern_bind(vm, *e2, *p2, binds))
}

/// `Blank[h]` 的 head 判定（legacy `expr_has_head`）。
pub(crate) fn expr_has_head(vm: &mut Vm<'_>, expr: TermId, head_pat: TermId) -> bool {
    let Some(Shape::Symbol(_)) = vm.shape(head_pat)
    else {
        return false;
    };
    let Some(name) = vm.head_name(head_pat)
    else {
        return false;
    };
    match name.as_str() {
        "Integer" => match vm.shape(expr) {
            Some(Shape::Number) => match vm.session.arena.get(expr) {
                Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) => {
                    n.as_exact_integer().is_some() || n.as_integer().is_some()
                }
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
