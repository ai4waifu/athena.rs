//! 微积分模块共享的项改写（arena 版 · Living `27` · `DomainExecutionContext`）。

use athena_types::TermId;

use crate::domains::context::DomainExecutionContext;
use crate::execution::shape::Shape;

/// 将 `var`（用户符号名）的每次出现替换为 `with`；未命中路径按 arena 共享。
pub(crate) fn replace_symbol(dc: &DomainExecutionContext<'_>, expr: TermId, var: &str, with: TermId) -> TermId {
    let var_id = dc.intern(var);
    replace_symbol_id(dc, expr, var_id, with)
}

fn replace_symbol_id(
    dc: &DomainExecutionContext<'_>,
    expr: TermId,
    var: athena_types::SymbolId,
    with: TermId,
) -> TermId {
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
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::Collection(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = replace_symbol_id(dc, i, var, with);
                changed |= r != i;
                out.push(r);
            }
            if changed { dc.ordered(out) } else { expr }
        }
        Shape::Application(op, args) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = replace_symbol_id(dc, a, var, with);
                changed |= r != a;
                out.push(r);
            }
            if changed { dc.apply_head(op, out) } else { expr }
        }
    }
}

/// `var` 是否在 `expr` 中自由出现。
pub(crate) fn contains_symbol(dc: &DomainExecutionContext<'_>, expr: TermId, var: &str) -> bool {
    let var_id = dc.intern(var);
    contains_symbol_id(dc, expr, var_id)
}

fn contains_symbol_id(dc: &DomainExecutionContext<'_>, expr: TermId, var: athena_types::SymbolId) -> bool {
    let Some(shape) = dc.shape(expr)
    else {
        return false;
    };
    match shape {
        Shape::Symbol(s) => dc.symbol_id_is(s, var),
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => false,
        Shape::Collection(items) => items.iter().any(|i| contains_symbol_id(dc, *i, var)),
        Shape::Application(_, args) => args.iter().any(|a| contains_symbol_id(dc, *a, var)),
    }
}
