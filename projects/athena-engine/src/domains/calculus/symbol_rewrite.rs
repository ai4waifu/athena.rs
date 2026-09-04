//! 微积分模块共享的项改写（arena 版 · 符号名键）。

use athena_types::TermId;

use super::ctx::CalculusCtx;
use crate::execution::vm::Shape;

/// 将 `var`（符号名）的每次出现替换为 `with`；未命中路径按 arena 共享。
pub(crate) fn replace_symbol(cc: &CalculusCtx<'_>, expr: TermId, var: &str, with: TermId) -> TermId {
    let Some(shape) = cc.shape(expr)
    else {
        return expr;
    };
    match shape {
        Shape::Sym(s) => {
            if cc.sym_is(s, var) {
                with
            }
            else {
                expr
            }
        }
        Shape::Number | Shape::Str(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::List(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for i in items {
                let r = replace_symbol(cc, i, var, with);
                changed |= r != i;
                out.push(r);
            }
            if changed { cc.list(out) } else { expr }
        }
        Shape::App(op, args) => {
            let head = cc.op_name(op).to_string();
            let mut changed = false;
            let mut out = Vec::with_capacity(args.len());
            for a in args {
                let r = replace_symbol(cc, a, var, with);
                changed |= r != a;
                out.push(r);
            }
            if changed { cc.ap(&head, out) } else { expr }
        }
    }
}

/// `var` 是否在 `expr` 中自由出现。
pub(crate) fn contains_symbol(cc: &CalculusCtx<'_>, expr: TermId, var: &str) -> bool {
    let Some(shape) = cc.shape(expr)
    else {
        return false;
    };
    match shape {
        Shape::Sym(s) => cc.sym_is(s, var),
        Shape::Number | Shape::Str(_) | Shape::Bool(_) | Shape::Null => false,
        Shape::List(items) => items.iter().any(|i| contains_symbol(cc, *i, var)),
        Shape::App(_, args) => args.iter().any(|a| contains_symbol(cc, *a, var)),
    }
}
