//! 符号规则改写预处理 — Own / Delayed / DownValues（legacy `apply_bindings` 语义）。
//!
//! 这是语义数据操作（rewriter 性质）：在编译前把当前 env 中可解析的符号替换掉，
//! 并在符号 head 上应用 DownValues。求值驱动与分派仍在编译单元与 VM。
//!
//! head 模型：`App{op, args}` 的 head 是注册表算子名；非符号 head 用
//! `Application[headTerm, args…]` 包装算子表示。

use std::collections::HashMap;

use athena_types::{ExprId, OperatorId, SymbolId};

use crate::execution::{
    Outcome,
    environment::definitions::{DefinitionLayer, LocalBinding},
    vm::{CompileMode, Shape, Vm},
};

/// 对子树做一轮规则改写（含 DownValues 递归应用）。
pub(crate) fn rewrite_bindings(vm: &mut Vm<'_>, expr: ExprId) -> ExprId {
    let Some(shape) = vm.shape(expr)
    else {
        return expr;
    };
    match shape {
        Shape::Sym(sym) => rewrite_symbol(vm, expr, sym),
        Shape::Number | Shape::Str(_) | Shape::Bool(_) | Shape::Null => expr,
        Shape::List(items) => {
            let mut changed = false;
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let r = rewrite_bindings(vm, item);
                changed |= r != item;
                out.push(r);
            }
            if !changed { expr } else { vm.push_list(out) }
        }
        Shape::App(op, args) => rewrite_app(vm, expr, op, args),
    }
}

fn rewrite_symbol(vm: &mut Vm<'_>, expr: ExprId, sym: SymbolId) -> ExprId {
    match vm.lookup_symbol(sym) {
        Some(LocalBinding::Own(v)) => v,
        Some(LocalBinding::Unique(v)) => v,
        None => expr,
    }
}

fn rewrite_app(vm: &mut Vm<'_>, expr: ExprId, op: OperatorId, args: Vec<ExprId>) -> ExprId {
    let name = vm.session.operators.name(op).unwrap_or("").to_string();

    // `Set` / `SetDelayed`：LHS 不改写，仅 RHS。
    if (name == "Set" || name == "SetDelayed") && args.len() == 2 {
        let rhs = rewrite_bindings(vm, args[1]);
        return if rhs == args[1] { expr } else { vm.rebuild_app_op(op, vec![args[0], rhs]) };
    }

    // 非符号 head 包装：改写 head 值与参数。
    if name == "Application" && !args.is_empty() {
        let head = rewrite_bindings(vm, args[0]);
        let mut new_args = vec![head];
        for a in &args[1..] {
            new_args.push(rewrite_bindings(vm, *a));
        }
        return if new_args == args { expr } else { vm.rebuild_app_wrapped(new_args) };
    }

    // 符号 head：参数先改写，再查 head 的 Own / Delayed（替换为值 → Application 包装），
    // 最后尝试 DownValues。
    let mut new_args = Vec::with_capacity(args.len());
    for a in &args {
        new_args.push(rewrite_bindings(vm, *a));
    }
    let unchanged = new_args == args;

    if let Some(value) = lookup_head_own(vm, op) {
        let mut wrapped = vec![value];
        wrapped.extend(new_args);
        let wrapped_id = vm.rebuild_app_wrapped(wrapped);
        return rewrite_bindings(vm, wrapped_id);
    }

    let app = if unchanged { expr } else { vm.rebuild_app_op(op, new_args) };
    apply_down_values(vm, app)
}

/// head 符号的 Own / Delayed 值（按注册表名反查符号表）。
fn lookup_head_own(vm: &mut Vm<'_>, op: OperatorId) -> Option<ExprId> {
    let name = vm.session.operators.name(op)?;
    let name = name.to_string();
    let sym = vm.session.arena.symbols_mut().intern(&name);
    match vm.lookup_symbol(sym) {
        Some(LocalBinding::Own(v)) => Some(v),
        Some(LocalBinding::Unique(v)) => Some(v),
        None => None,
    }
}

/// 对已改写的 App 尝试 DownValues（首条匹配规则获胜，结果递归改写）。
fn apply_down_values(vm: &mut Vm<'_>, app: ExprId) -> ExprId {
    let Some(Shape::App(op, args)) = vm.shape(app)
    else {
        return app;
    };
    let name = match vm.session.operators.name(op) {
        Some(n) => n.to_string(),
        None => return app,
    };
    if name == "Application" {
        return app;
    }
    let sym = vm.session.arena.symbols_mut().intern(&name);
    let Some(rules) = vm.down_values(sym)
    else {
        return app;
    };
    for (lhs, rhs) in rules {
        let Some(Shape::App(_, pat_args)) = vm.shape(lhs)
        else {
            continue;
        };
        if pat_args.len() != args.len() {
            continue;
        }
        let mut binds: HashMap<SymbolId, ExprId> = HashMap::new();
        if pat_args
            .iter()
            .zip(args.iter())
            .all(|(p, a)| crate::execution::builtins::patterns::pattern_bind(vm, *a, *p, &mut binds))
        {
            let substituted = crate::execution::builtins::patterns::substitute_binds(vm, rhs, &binds);
            return rewrite_bindings(vm, substituted);
        }
    }
    app
}
