//! 微积分模块共享的项改写（arena 版 · · `DomainExecutionContext`）。

use athena_types::{SymbolId, TermId};

use crate::{domains::context::DomainExecutionContext, execution::shape::Shape};

/// 将 `var` 的每次出现替换为 `with`；未命中路径按 arena 共享。
pub(crate) fn replace_symbol(dc: &DomainExecutionContext<'_>, expr: TermId, var: SymbolId, with: TermId) -> TermId {
    let Some(shape) = dc.shape(expr)
    else {
        return expr;
    };
    match shape {
        Shape::Symbol(s) => {
            if dc.symbol_id_is(s, var) {
                with
            }
            else {
                expr
            }
        }
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => expr,
        Shape::Collection(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = replace_symbol(dc, i, var, with);
                changed |= r != i;
                out.push(r);
            }
            if changed { dc.ordered(out) } else { expr }
        }
        Shape::Application(op, args) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = replace_symbol(dc, a, var, with);
                changed |= r != a;
                out.push(r);
            }
            if changed { dc.apply_head(op, out) } else { expr }
        }
    }
}

/// `var` 是否在 `expr` 中自由出现。
pub(crate) fn contains_symbol(dc: &DomainExecutionContext<'_>, expr: TermId, var: SymbolId) -> bool {
    let Some(shape) = dc.shape(expr)
    else {
        return false;
    };
    match shape {
        Shape::Symbol(s) => dc.symbol_id_is(s, var),
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null | Shape::Constant(_) => false,
        Shape::Collection(items) => items.iter().any(|i| contains_symbol(dc, *i, var)),
        Shape::Application(_, args) => args.iter().any(|a| contains_symbol(dc, *a, var)),
    }
}

/// 项是否为用户符号 `var`。
pub(crate) fn is_symbol_id(dc: &DomainExecutionContext<'_>, term: TermId, var: SymbolId) -> bool {
    matches!(dc.shape(term), Some(Shape::Symbol(s)) if dc.symbol_id_is(s, var))
}
