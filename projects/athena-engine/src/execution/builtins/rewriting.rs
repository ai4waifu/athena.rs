//! 符号规则改写预处理 — Own / Delayed / DownValues（legacy `apply_bindings` 语义）。
//!
//! 这是语义数据操作（rewriter 性质）：在编译前把当前 env 中可解析的符号替换掉，
//! 并在符号 head 上应用 DownValues。求值驱动与分派仍在编译单元与 VM。
//!
//! head 模型：`App{op, args}` 的 head 是注册表算子名；非符号 head 用
//! `Application[headTerm, args…]` 包装算子表示。

use std::collections::HashMap;

use athena_types::{OperatorId, SymbolId, TermId};

use crate::execution::{
    TermEvaluation,
    environment::definitions::{DefinitionLayer, LocalBinding},
    vm::{CompileMode, Shape, Vm},
};

/// 对子树做一轮规则改写（含 DownValues 递归应用）。
pub(crate) fn rewrite_bindings(vm: &mut Vm<'_>, expr: TermId) -> TermId {
    let Some(shape) = vm.shape(expr)
    else {
        return expr;
    };
    match shape {
        Shape::Symbol(symbol) => rewrite_symbol(vm, expr, symbol),
        Shape::Number | Shape::String(_) | Shape::Bool(_) | Shape::Null => expr,
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
        Shape::Application(op, args) => rewrite_application(vm, expr, op, args),
    }
}

fn rewrite_symbol(vm: &mut Vm<'_>, expr: TermId, symbol: SymbolId) -> TermId {
    match vm.lookup_symbol(symbol) {
        Some(LocalBinding::Own(v)) => v,
        Some(LocalBinding::Unique(v)) => v,
        None => expr,
    }
}

fn rewrite_application(vm: &mut Vm<'_>, expr: TermId, op: OperatorId, args: Vec<TermId>) -> TermId {
    let name = vm.session.operators.name(op).unwrap_or("").to_string();

    // `Define` / `DefineDeferred`：LHS 不改写，仅 RHS。
    if (name == "Define" || name == "DefineDeferred") && args.len() == 2 {
        let rhs = rewrite_bindings(vm, args[1]);
        return if rhs == args[1] { expr } else { vm.rebuild_application_operator(op, vec![args[0], rhs]) };
    }

    // 非符号 head 包装：改写 head 值与参数。
    if name == "Application" && !args.is_empty() {
        let head = rewrite_bindings(vm, args[0]);
        let mut new_args = vec![head];
        for a in &args[1..] {
            new_args.push(rewrite_bindings(vm, *a));
        }
        return if new_args == args { expr } else { vm.rebuild_application_wrapped(new_args) };
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
        let wrapped_id = vm.rebuild_application_wrapped(wrapped);
        return rewrite_bindings(vm, wrapped_id);
    }

    let app = if unchanged { expr } else { vm.rebuild_application_operator(op, new_args) };
    apply_down_values(vm, app)
}

/// head 符号的 Own / Delayed 值（按注册表名反查符号表）。
fn lookup_head_own(vm: &mut Vm<'_>, op: OperatorId) -> Option<TermId> {
    let name = vm.session.operators.name(op)?;
    let name = name.to_string();
    let symbol = vm.session.arena.symbols_mut().intern(&name);
    match vm.lookup_symbol(symbol) {
        Some(LocalBinding::Own(v)) => Some(v),
        Some(LocalBinding::Unique(v)) => Some(v),
        None => None,
    }
}

/// 对已改写的 App 尝试 DownValues（首条匹配规则获胜，结果递归改写）。
fn apply_down_values(vm: &mut Vm<'_>, app: TermId) -> TermId {
    let Some(Shape::Application(op, args)) = vm.shape(app)
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
    let symbol = vm.session.arena.symbols_mut().intern(&name);
    let Some(rules) = vm.down_values(symbol)
    else {
        return app;
    };
    for (lhs, rhs) in rules {
        let Some(Shape::Application(_, pat_args)) = vm.shape(lhs)
        else {
            continue;
        };
        if pat_args.len() != args.len() {
            continue;
        }
        let mut binds: HashMap<SymbolId, TermId> = HashMap::new();
        if pat_args.iter().zip(args.iter()).all(|(p, a)| crate::execution::builtins::patterns::pattern_bind(vm, *a, *p, &mut binds)) {
            let substituted = crate::execution::builtins::patterns::substitute_binds(vm, rhs, &binds);
            return rewrite_bindings(vm, substituted);
        }
    }
    app
}
