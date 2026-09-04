//! 控制流与作用域 handler（legacy `eval_compound*` / `eval_if` / `eval_while` / `eval_local_scope` 等语义）。

use athena_ir::{Atom, ExprNode};
use athena_types::ExprId;

use crate::execution::{
    Outcome,
    builtins::catalog::{as_boolean_id, expression_summary, non_boolean_condition_diagnostic},
    environment::{LocalBinding, ScopeFrame},
    vm::Vm,
};

/// `Function[…][args…]` 动态分派（`EvalDynamic`）。
pub(crate) fn eval_dynamic(vm: &mut Vm<'_>, head: ExprId, args: Vec<ExprId>) -> Outcome {
    if vm.head_name(head).is_some_and(|h| h == "Function") {
        return apply_function(vm, &vm.app_args(head).unwrap_or_default(), &args);
    }
    Outcome::unevaluated(vm.rebuild_app(head, args))
}

/// `CompoundExpression` 语句位：顺序求值，写当前 env。
pub(crate) fn h_compound(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    compound_into(vm, args)
}

/// `CompoundExpression` 值位：全新 env（legacy `eval_compound`）。
pub(crate) fn h_compound_fresh(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    vm.push_env_fresh();
    let out = compound_into(vm, args);
    vm.pop_env();
    out
}

fn compound_into(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.is_empty() {
        return Outcome::value(vm.push_null());
    }
    let mut diags = Vec::new();
    let mut last = vm.push_null();
    for arg in args {
        let o = vm.eval_stmt(*arg);
        diags.extend(o.diagnostics);
        last = o.term;
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            return Outcome {
                term: last,
                kind: crate::execution::EvalKind::Unevaluated,
                status: athena_types::ComputationStatus::Invalid,
                diagnostics: diags,
            };
        }
    }
    Outcome {
        term: last,
        kind: crate::execution::EvalKind::Value,
        status: athena_types::ComputationStatus::Exact,
        diagnostics: diags,
    }
}

pub(crate) fn h_if(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.len() < 2 || args.len() > 4 {
        return Outcome::unevaluated(vm.push_app("If", args.to_vec()));
    }
    let cond_o = vm.eval_value(args[0]);
    let mut diags = cond_o.diagnostics.clone();
    match as_boolean_id(vm, cond_o.term) {
        Some(true) => vm.eval_value(args[1]).with_diagnostics(diags),
        Some(false) => {
            if args.len() >= 3 {
                vm.eval_value(args[2]).with_diagnostics(diags)
            }
            else {
                Outcome {
                    term: vm.push_null(),
                    kind: crate::execution::EvalKind::Value,
                    status: athena_types::ComputationStatus::Exact,
                    diagnostics: diags,
                }
            }
        }
        None => {
            if args.len() == 4 {
                vm.eval_value(args[3]).with_diagnostics(diags)
            }
            else {
                let summary = expression_summary(vm, cond_o.term);
                let mut held = vec![cond_o.term];
                held.extend_from_slice(&args[1..]);
                let term = vm.push_app("If", held);
                diags.push(non_boolean_condition_diagnostic(&summary));
                Outcome {
                    term,
                    kind: crate::execution::EvalKind::Unevaluated,
                    status: athena_types::ComputationStatus::Invalid,
                    diagnostics: diags,
                }
            }
        }
    }
}

pub(crate) fn h_which(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.is_empty() || args.len() % 2 != 0 {
        return Outcome::unevaluated(vm.push_app("Which", args.to_vec()));
    }
    let mut diags = Vec::new();
    let mut uneval_pairs: Vec<ExprId> = Vec::new();
    let mut i = 0;
    while i + 1 < args.len() {
        let cond_o = vm.eval_value(args[i]);
        diags.extend(cond_o.diagnostics.clone());
        match as_boolean_id(vm, cond_o.term) {
            Some(true) => {
                return vm.eval_value(args[i + 1]).with_diagnostics(diags);
            }
            Some(false) => {}
            None => {
                uneval_pairs.push(cond_o.term);
                uneval_pairs.push(args[i + 1]);
            }
        }
        i += 2;
    }
    if uneval_pairs.is_empty() {
        Outcome {
            term: vm.push_null(),
            kind: crate::execution::EvalKind::Value,
            status: athena_types::ComputationStatus::Exact,
            diagnostics: diags,
        }
    }
    else {
        let summary = expression_summary(vm, uneval_pairs[0]);
        diags.push(non_boolean_condition_diagnostic(&summary));
        Outcome {
            term: vm.push_app("Which", uneval_pairs),
            kind: crate::execution::EvalKind::Unevaluated,
            status: athena_types::ComputationStatus::Invalid,
            diagnostics: diags,
        }
    }
}

/// `While[cond, body]`（语句位 · 继承 env）。
pub(crate) fn h_while(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    while_loop(vm, args)
}

/// `While` 值位：全新 env。
pub(crate) fn h_while_fresh(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    vm.push_env_fresh();
    let out = while_loop(vm, args);
    vm.pop_env();
    out
}

fn while_loop(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app("While", args.to_vec()));
    }
    let mut diags = Vec::new();
    let mut last = vm.push_null();
    let mut ran = false;
    for _ in 0..1024u32 {
        let cond_o = vm.eval_value(args[0]);
        diags.extend(cond_o.diagnostics.clone());
        match as_boolean_id(vm, cond_o.term) {
            Some(false) => {
                let term = if ran { last } else { vm.push_null() };
                return Outcome {
                    term,
                    kind: crate::execution::EvalKind::Value,
                    status: athena_types::ComputationStatus::Exact,
                    diagnostics: diags,
                };
            }
            Some(true) => {
                ran = true;
                let body_o = vm.eval_stmt(args[1]);
                diags.extend(body_o.diagnostics);
                last = body_o.term;
                if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
                    return Outcome {
                        term: last,
                        kind: crate::execution::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
            }
            None => {
                diags.push(non_boolean_condition_diagnostic(&expression_summary(vm, cond_o.term)));
                let term = vm.push_app("While", vec![cond_o.term, args[1]]);
                return Outcome {
                    term,
                    kind: crate::execution::EvalKind::Unevaluated,
                    status: athena_types::ComputationStatus::Invalid,
                    diagnostics: diags,
                };
            }
        }
    }
    let term = vm.push_app("While", args.to_vec());
    diags.push(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation).detail("operation", "While"));
    Outcome {
        term,
        kind: crate::execution::EvalKind::Unevaluated,
        status: athena_types::ComputationStatus::Invalid,
        diagnostics: diags,
    }
}

/// `For[var, iterator, body]`（语句位 · 继承 env）。
pub(crate) fn h_for(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    for_loop(vm, args)
}

/// `For` 值位：全新 env。
pub(crate) fn h_for_fresh(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    vm.push_env_fresh();
    let out = for_loop(vm, args);
    vm.pop_env();
    out
}

fn for_loop(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.len() != 3 {
        return Outcome::unevaluated(vm.push_app("For", args.to_vec()));
    }
    let var_sym = match vm.session.arena.get(args[0]) {
        Some(ExprNode::Atom(Atom::Symbol(s))) => *s,
        _ => return Outcome::unevaluated(vm.push_app("For", args.to_vec())),
    };
    let iter_o = vm.eval_value(args[1]);
    let mut diags = iter_o.diagnostics.clone();
    let Some(values) = (match vm.session.arena.get(iter_o.term) {
        Some(ExprNode::List(_)) => vm.app_args(iter_o.term),
        _ => None,
    })
    else {
        let term = vm.push_app("For", vec![args[0], iter_o.term, args[2]]);
        return Outcome::unevaluated(term);
    };
    let mut last = vm.push_null();
    for value in values {
        let body = crate::execution::builtins::patterns::substitute_symbol(vm, args[2], var_sym, value);
        let body_o = vm.eval_stmt(body);
        diags.extend(body_o.diagnostics);
        last = body_o.term;
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            return Outcome {
                term: last,
                kind: crate::execution::EvalKind::Unevaluated,
                status: athena_types::ComputationStatus::Invalid,
                diagnostics: diags,
            };
        }
    }
    Outcome {
        term: last,
        kind: crate::execution::EvalKind::Value,
        status: athena_types::ComputationStatus::Exact,
        diagnostics: diags,
    }
}

/// `Try[body, catch]`：body Error 时求值 catch。
pub(crate) fn h_try(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app("Try", args.to_vec()));
    }
    let body_o = vm.eval_value(args[0]);
    if body_o.has_error() {
        return vm.eval_value(args[1]);
    }
    body_o
}

// ---- Hold / Pattern 保持 ----

pub(crate) fn h_hold(vm: &mut Vm<'_>, operands: &[ExprId]) -> Outcome {
    let _ = vm;
    Outcome::unevaluated(operands[0])
}

pub(crate) fn h_pattern_hold(vm: &mut Vm<'_>, operands: &[ExprId]) -> Outcome {
    let _ = vm;
    Outcome::unevaluated(operands[0])
}

// ---- With / Module / Block ----

pub(crate) fn h_with(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    with_block(vm, args, false, "With")
}

pub(crate) fn h_with_top(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    with_block(vm, args, true, "With")
}

pub(crate) fn h_block(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    with_block(vm, args, false, "Block")
}

pub(crate) fn h_block_top(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    with_block(vm, args, true, "Block")
}

/// `With` / `Block`：局部 `{x=1,…}` 后求值体（legacy `eval_local_scope` 非路径）。
fn with_block(vm: &mut Vm<'_>, args: &[ExprId], top: bool, head: &str) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app(head, args.to_vec()));
    }
    let Some(locals) = (match vm.session.arena.get(args[0]) {
        Some(ExprNode::List(_)) => vm.app_args(args[0]),
        _ => None,
    })
    else {
        let term = vm.push_app(head, vec![args[0], args[1]]);
        return Outcome::unevaluated(term);
    };
    let mut frame = ScopeFrame::new();
    let mut diags = Vec::new();
    for item in locals {
        let Some((name, rhs)) = match_set(vm, item)
        else {
            return Outcome::unevaluated(vm.push_app(head, args.to_vec()));
        };
        let o = vm.eval_value(rhs);
        diags.extend(o.diagnostics);
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            let term = vm.push_app(head, args.to_vec());
            return Outcome {
                term,
                kind: crate::execution::EvalKind::Unevaluated,
                status: athena_types::ComputationStatus::Invalid,
                diagnostics: diags,
            };
        }
        frame.bind(name, LocalBinding::Own(o.term));
    }
    vm.push_env_scoped(frame, top);
    let out = vm.eval_value(args[1]);
    vm.pop_env();
    out.with_diagnostics(diags)
}

/// `x = rhs` 语句形态识别 → `(SymbolId, rhs)`。
fn match_set(vm: &Vm<'_>, term: ExprId) -> Option<(athena_types::SymbolId, ExprId)> {
    let ExprNode::App { args, .. } = vm.session.arena.get(term)?
    else {
        return None;
    };
    if args.len() != 2 || vm.head_name(term).is_none_or(|h| h != "Set") {
        return None;
    }
    let sym = match vm.session.arena.get(args[0]) {
        Some(ExprNode::Atom(Atom::Symbol(s))) => *s,
        _ => return None,
    };
    Some((sym, args[1]))
}

pub(crate) fn h_module(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    module(vm, args, false)
}

pub(crate) fn h_module_top(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    module(vm, args, true)
}

/// `Module[{x=1, y}, body]`：局部符号帧（未初始化逃逸物化为 `name$N`）。
fn module(vm: &mut Vm<'_>, args: &[ExprId], top: bool) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app("Module", args.to_vec()));
    }
    let Some(locals) = (match vm.session.arena.get(args[0]) {
        Some(ExprNode::List(_)) => vm.app_args(args[0]),
        _ => None,
    })
    else {
        let term = vm.push_app("Module", vec![args[0], args[1]]);
        return Outcome::unevaluated(term);
    };

    // 初始化阶段：顺序求值，先前局部以原名可见（legacy `init_env` 语义）。
    let mut init_frame = ScopeFrame::new();
    let mut diags = Vec::new();
    let mut initialized: Vec<(athena_types::SymbolId, ExprId)> = Vec::new();
    let mut bare: Vec<athena_types::SymbolId> = Vec::new();
    for item in locals.clone() {
        let local = match module_local(vm, item) {
            Some(l) => l,
            None => {
                let term = vm.push_app("Module", vec![args[0], args[1]]);
                return Outcome::unevaluated(term);
            }
        };
        match local {
            (sym, Some(rhs)) => {
                vm.push_env_scoped(init_frame.clone(), top);
                let o = vm.eval_value(rhs);
                vm.pop_env();
                diags.extend(o.diagnostics);
                if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
                    let term = vm.push_app("Module", vec![args[0], args[1]]);
                    return Outcome {
                        term,
                        kind: crate::execution::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
                init_frame.bind(sym, LocalBinding::Own(o.term));
                initialized.push((sym, o.term));
            }
            (sym, None) => bare.push(sym),
        }
    }

    // 体阶段：初始化局部 → 值；未初始化局部 → 唯一化符号。
    let mut body_frame = ScopeFrame::new();
    for (sym, value) in initialized {
        body_frame.bind(sym, LocalBinding::Own(value));
    }
    for sym in bare {
        let name = vm.session.arena.symbols().resolve(sym).unwrap_or("?").to_string();
        let uniq = vm.unique_symbol(&name);
        body_frame.bind(sym, LocalBinding::Unique(uniq));
    }
    vm.push_env_scoped(body_frame, top);
    let out = vm.eval_value(args[1]);
    vm.pop_env();
    out.with_diagnostics(diags)
}

/// `Module` 局部：`x=1` 或裸 `x`（legacy `match_module_local`）。
fn module_local(vm: &Vm<'_>, term: ExprId) -> Option<(athena_types::SymbolId, Option<ExprId>)> {
    if let Some((sym, rhs)) = match_set(vm, term) {
        return Some((sym, Some(rhs)));
    }
    match vm.session.arena.get(term) {
        Some(ExprNode::Atom(Atom::Symbol(s))) => Some((*s, None)),
        _ => None,
    }
}

/// `Function[body]`（Slot）或 `Function[var, body]` 应用（legacy `apply_function`）。
pub(crate) fn apply_function(vm: &mut Vm<'_>, fargs: &[ExprId], args: &[ExprId]) -> Outcome {
    match fargs {
        [body] if args.len() == 1 => {
            let substituted = crate::execution::builtins::patterns::substitute_slot(vm, *body, args[0]);
            vm.eval_value(substituted)
        }
        [var, body] if args.len() == 1 => {
            if let Some(sym) = match vm.session.arena.get(*var) {
                Some(ExprNode::Atom(Atom::Symbol(s))) => Some(*s),
                _ => None,
            } {
                let substituted = crate::execution::builtins::patterns::substitute_symbol(vm, *body, sym, args[0]);
                vm.eval_value(substituted)
            }
            else {
                let head = vm.push_app("Function", fargs.to_vec());
                Outcome::unevaluated(vm.rebuild_app(head, args.to_vec()))
            }
        }
        _ => {
            let head = vm.push_app("Function", fargs.to_vec());
            Outcome::unevaluated(vm.rebuild_app(head, args.to_vec()))
        }
    }
}
