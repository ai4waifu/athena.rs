//! 控制流与作用域 handler（legacy `eval_compound*` / `eval_if` / `eval_while` / `eval_local_scope` 等语义）。

use athena_ir::{Atom, TermNode};
use athena_types::TermId;

use crate::execution::{
    TermEvaluation,
    builtins::catalog::{as_boolean_id, non_boolean_condition_diagnostic, term_summary},
    environment::{LocalBinding, ScopeFrame},
    vm::Vm,
};

/// `Function[…][args…]` 动态分派（`EvalDynamic`）。
pub(crate) fn eval_dynamic(vm: &mut Vm<'_>, head: TermId, args: Vec<TermId>) -> TermEvaluation {
    if vm.head_name(head).is_some_and(|h| h == "Function") {
        return apply_function(vm, &vm.application_arguments(head).unwrap_or_default(), &args);
    }
    TermEvaluation::unevaluated(vm.rebuild_application(head, args))
}

/// `CompoundExpression` 语句位：顺序求值，写当前 env。
pub(crate) fn h_compound(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    compound_into(vm, args)
}

/// `CompoundExpression` 值位：全新 env（legacy `eval_compound`）。
pub(crate) fn h_compound_fresh(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    vm.push_env_fresh();
    let out = compound_into(vm, args);
    vm.pop_env();
    out
}

fn compound_into(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    if args.is_empty() {
        return TermEvaluation::value(vm.push_null());
    }
    let mut diags = Vec::new();
    let mut last = vm.push_null();
    for arg in args {
        let o = vm.eval_stmt(*arg);
        diags.extend(o.diagnostics);
        last = o.term;
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            return TermEvaluation {
                term: last,
                kind: crate::execution::EvalKind::Unevaluated,
                status: athena_types::ComputationStatus::Invalid,
                diagnostics: diags,
            };
        }
    }
    TermEvaluation { term: last, kind: crate::execution::EvalKind::Value, status: athena_types::ComputationStatus::Exact, diagnostics: diags }
}

pub(crate) fn h_if(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    if args.len() < 2 || args.len() > 4 {
        return TermEvaluation::unevaluated(vm.push_application("If", args.to_vec()));
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
                TermEvaluation {
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
                let summary = term_summary(vm, cond_o.term);
                let mut held = vec![cond_o.term];
                held.extend_from_slice(&args[1..]);
                let term = vm.push_application("If", held);
                diags.push(non_boolean_condition_diagnostic(&summary));
                TermEvaluation {
                    term,
                    kind: crate::execution::EvalKind::Unevaluated,
                    status: athena_types::ComputationStatus::Invalid,
                    diagnostics: diags,
                }
            }
        }
    }
}

pub(crate) fn h_which(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    if args.is_empty() || args.len() % 2 != 0 {
        return TermEvaluation::unevaluated(vm.push_application("Which", args.to_vec()));
    }
    let mut diags = Vec::new();
    let mut uneval_pairs: Vec<TermId> = Vec::new();
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
        TermEvaluation {
            term: vm.push_null(),
            kind: crate::execution::EvalKind::Value,
            status: athena_types::ComputationStatus::Exact,
            diagnostics: diags,
        }
    }
    else {
        let summary = term_summary(vm, uneval_pairs[0]);
        diags.push(non_boolean_condition_diagnostic(&summary));
        TermEvaluation {
            term: vm.push_application("Which", uneval_pairs),
            kind: crate::execution::EvalKind::Unevaluated,
            status: athena_types::ComputationStatus::Invalid,
            diagnostics: diags,
        }
    }
}

/// `While[cond, body]`（语句位 · 继承 env）。
pub(crate) fn h_while(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    while_loop(vm, args)
}

/// `While` 值位：全新 env。
pub(crate) fn h_while_fresh(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    vm.push_env_fresh();
    let out = while_loop(vm, args);
    vm.pop_env();
    out
}

fn while_loop(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    if args.len() != 2 {
        return TermEvaluation::unevaluated(vm.push_application("While", args.to_vec()));
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
                return TermEvaluation {
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
                    return TermEvaluation {
                        term: last,
                        kind: crate::execution::EvalKind::Unevaluated,
                        status: athena_types::ComputationStatus::Invalid,
                        diagnostics: diags,
                    };
                }
            }
            None => {
                diags.push(non_boolean_condition_diagnostic(&term_summary(vm, cond_o.term)));
                let term = vm.push_application("While", vec![cond_o.term, args[1]]);
                return TermEvaluation {
                    term,
                    kind: crate::execution::EvalKind::Unevaluated,
                    status: athena_types::ComputationStatus::Invalid,
                    diagnostics: diags,
                };
            }
        }
    }
    let term = vm.push_application("While", args.to_vec());
    diags.push(athena_types::Diagnostic::new(athena_types::DiagnosticCode::UnsupportedOperation).detail("operation", "While"));
    TermEvaluation { term, kind: crate::execution::EvalKind::Unevaluated, status: athena_types::ComputationStatus::Invalid, diagnostics: diags }
}

/// `For[var, iterator, body]`（语句位 · 继承 env）。
pub(crate) fn h_for(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    for_loop(vm, args)
}

/// `For` 值位：全新 env。
pub(crate) fn h_for_fresh(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    vm.push_env_fresh();
    let out = for_loop(vm, args);
    vm.pop_env();
    out
}

fn for_loop(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    if args.len() != 3 {
        return TermEvaluation::unevaluated(vm.push_application("For", args.to_vec()));
    }
    let var_sym = match vm.session.arena.get(args[0]) {
        Some(TermNode::Atom(Atom::Symbol(s))) => *s,
        _ => return TermEvaluation::unevaluated(vm.push_application("For", args.to_vec())),
    };
    let iter_o = vm.eval_value(args[1]);
    let mut diags = iter_o.diagnostics.clone();
    let Some(values) = (match vm.session.arena.get(iter_o.term) {
        Some(TermNode::List(_)) => vm.application_arguments(iter_o.term),
        _ => None,
    })
    else {
        let term = vm.push_application("For", vec![args[0], iter_o.term, args[2]]);
        return TermEvaluation::unevaluated(term);
    };
    let mut last = vm.push_null();
    for value in values {
        let body = crate::execution::builtins::patterns::substitute_symbol(vm, args[2], var_sym, value);
        let body_o = vm.eval_stmt(body);
        diags.extend(body_o.diagnostics);
        last = body_o.term;
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            return TermEvaluation {
                term: last,
                kind: crate::execution::EvalKind::Unevaluated,
                status: athena_types::ComputationStatus::Invalid,
                diagnostics: diags,
            };
        }
    }
    TermEvaluation { term: last, kind: crate::execution::EvalKind::Value, status: athena_types::ComputationStatus::Exact, diagnostics: diags }
}

/// `Try[body, catch]`：body Error 时求值 catch。
pub(crate) fn h_try(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    if args.len() != 2 {
        return TermEvaluation::unevaluated(vm.push_application("Try", args.to_vec()));
    }
    let body_o = vm.eval_value(args[0]);
    if body_o.has_error() {
        return vm.eval_value(args[1]);
    }
    body_o
}

// ---- Hold / Pattern 保持 ----

pub(crate) fn h_hold(vm: &mut Vm<'_>, operands: &[TermId]) -> TermEvaluation {
    let _ = vm;
    TermEvaluation::unevaluated(operands[0])
}

pub(crate) fn h_pattern_hold(vm: &mut Vm<'_>, operands: &[TermId]) -> TermEvaluation {
    let _ = vm;
    TermEvaluation::unevaluated(operands[0])
}

// ---- With / Module / Block ----

pub(crate) fn h_with(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    with_block(vm, args, false, "With")
}

pub(crate) fn h_with_top(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    with_block(vm, args, true, "With")
}

pub(crate) fn h_block(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    with_block(vm, args, false, "Block")
}

pub(crate) fn h_block_top(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    with_block(vm, args, true, "Block")
}

/// `With` / `Block`：局部 `{x=1,…}` 后求值体（legacy `eval_local_scope` 非路径）。
fn with_block(vm: &mut Vm<'_>, args: &[TermId], top: bool, head: &str) -> TermEvaluation {
    if args.len() != 2 {
        return TermEvaluation::unevaluated(vm.push_application(head, args.to_vec()));
    }
    let Some(locals) = (match vm.session.arena.get(args[0]) {
        Some(TermNode::List(_)) => vm.application_arguments(args[0]),
        _ => None,
    })
    else {
        let term = vm.push_application(head, vec![args[0], args[1]]);
        return TermEvaluation::unevaluated(term);
    };
    let mut frame = ScopeFrame::new();
    let mut diags = Vec::new();
    for item in locals {
        let Some((name, rhs)) = match_set(vm, item)
        else {
            return TermEvaluation::unevaluated(vm.push_application(head, args.to_vec()));
        };
        let o = vm.eval_value(rhs);
        diags.extend(o.diagnostics);
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            let term = vm.push_application(head, args.to_vec());
            return TermEvaluation {
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
fn match_set(vm: &Vm<'_>, term: TermId) -> Option<(athena_types::SymbolId, TermId)> {
    let TermNode::Application { arguments: args, .. } = vm.session.arena.get(term)?
    else {
        return None;
    };
    if args.len() != 2 || vm.head_name(term).is_none_or(|h| h != "Set") {
        return None;
    }
    let sym = match vm.session.arena.get(args[0]) {
        Some(TermNode::Atom(Atom::Symbol(s))) => *s,
        _ => return None,
    };
    Some((sym, args[1]))
}

pub(crate) fn h_module(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    module(vm, args, false)
}

pub(crate) fn h_module_top(vm: &mut Vm<'_>, args: &[TermId]) -> TermEvaluation {
    module(vm, args, true)
}

/// `Module[{x=1, y}, body]`：局部符号帧（未初始化逃逸物化为 `name$N`）。
fn module(vm: &mut Vm<'_>, args: &[TermId], top: bool) -> TermEvaluation {
    if args.len() != 2 {
        return TermEvaluation::unevaluated(vm.push_application("Module", args.to_vec()));
    }
    let Some(locals) = (match vm.session.arena.get(args[0]) {
        Some(TermNode::List(_)) => vm.application_arguments(args[0]),
        _ => None,
    })
    else {
        let term = vm.push_application("Module", vec![args[0], args[1]]);
        return TermEvaluation::unevaluated(term);
    };

    // 初始化阶段：顺序求值，先前局部以原名可见（legacy `init_env` 语义）。
    let mut init_frame = ScopeFrame::new();
    let mut diags = Vec::new();
    let mut initialized: Vec<(athena_types::SymbolId, TermId)> = Vec::new();
    let mut bare: Vec<athena_types::SymbolId> = Vec::new();
    for item in locals.clone() {
        let local = match module_local(vm, item) {
            Some(l) => l,
            None => {
                let term = vm.push_application("Module", vec![args[0], args[1]]);
                return TermEvaluation::unevaluated(term);
            }
        };
        match local {
            (sym, Some(rhs)) => {
                vm.push_env_scoped(init_frame.clone(), top);
                let o = vm.eval_value(rhs);
                vm.pop_env();
                diags.extend(o.diagnostics);
                if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
                    let term = vm.push_application("Module", vec![args[0], args[1]]);
                    return TermEvaluation {
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
fn module_local(vm: &Vm<'_>, term: TermId) -> Option<(athena_types::SymbolId, Option<TermId>)> {
    if let Some((sym, rhs)) = match_set(vm, term) {
        return Some((sym, Some(rhs)));
    }
    match vm.session.arena.get(term) {
        Some(TermNode::Atom(Atom::Symbol(s))) => Some((*s, None)),
        _ => None,
    }
}

/// `Function[body]`（Slot）或 `Function[var, body]` 应用（legacy `apply_function`）。
pub(crate) fn apply_function(vm: &mut Vm<'_>, fargs: &[TermId], args: &[TermId]) -> TermEvaluation {
    match fargs {
        [body] if args.len() == 1 => {
            let substituted = crate::execution::builtins::patterns::substitute_slot(vm, *body, args[0]);
            vm.eval_value(substituted)
        }
        [var, body] if args.len() == 1 => {
            if let Some(sym) = match vm.session.arena.get(*var) {
                Some(TermNode::Atom(Atom::Symbol(s))) => Some(*s),
                _ => None,
            } {
                let substituted = crate::execution::builtins::patterns::substitute_symbol(vm, *body, sym, args[0]);
                vm.eval_value(substituted)
            }
            else {
                let head = vm.push_application("Function", fargs.to_vec());
                TermEvaluation::unevaluated(vm.rebuild_application(head, args.to_vec()))
            }
        }
        _ => {
            let head = vm.push_application("Function", fargs.to_vec());
            TermEvaluation::unevaluated(vm.rebuild_application(head, args.to_vec()))
        }
    }
}
