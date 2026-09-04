//! Table / Sum / Product / MatchQ / Cases / Function handler（legacy 对应语义）。

use athena_ir::{Atom, ExprNode};
use athena_types::ExprId;

use crate::execution::{
    Outcome,
    builtins::{
        arithmetic::number_of,
        catalog::{expression_summary, non_boolean_condition_diagnostic},
    },
    vm::Vm,
};

pub(crate) fn h_table(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    table(vm, args).0
}

/// `Table[body, {i, n} | {i, a, b} | {i, a, b, step} | {n}]` — body HoldAll-ish。
/// 返回 (出口, 可选展开值 — `Sum` / `Product` 复用)。
pub(crate) fn table(vm: &mut Vm<'_>, args: &[ExprId]) -> (Outcome, Option<Vec<ExprId>>) {
    if args.len() != 2 {
        return (Outcome::unevaluated(vm.push_app("Table", args.to_vec())), None);
    }
    let iter_o = vm.eval_value(args[1]);
    let mut diags = iter_o.diagnostics.clone();
    let Some((var, values)) = expand_iterator(vm, iter_o.term)
    else {
        let term = vm.push_app("Table", vec![args[0], iter_o.term]);
        return (Outcome::unevaluated(term), None);
    };
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        let body = match var {
            Some(sym) => crate::execution::builtins::patterns::substitute_symbol(vm, args[0], sym, value),
            None => args[0],
        };
        let body_o = vm.eval_value(body);
        diags.extend(body_o.diagnostics);
        out.push(body_o.term);
        if diags.iter().any(|d| d.severity == athena_types::Severity::Error) {
            return (
                Outcome {
                    term: vm.push_list(out),
                    kind: crate::execution::EvalKind::Unevaluated,
                    status: athena_types::ComputationStatus::Invalid,
                    diagnostics: diags,
                },
                None,
            );
        }
    }
    let term = vm.push_list(out.clone());
    (
        Outcome {
            term,
            kind: crate::execution::EvalKind::Value,
            status: athena_types::ComputationStatus::Exact,
            diagnostics: diags,
        },
        Some(out),
    )
}

/// 展开 `{i,n}` / `{i,a,b}` / `{i,a,b,step}` / `{n}` → (可选符号, 值表)。
fn expand_iterator(vm: &mut Vm<'_>, spec: ExprId) -> Option<(Option<athena_types::SymbolId>, Vec<ExprId>)> {
    let items = match vm.session.arena.get(spec) {
        Some(ExprNode::List(_)) => vm.app_args(spec)?,
        _ => return None,
    };
    match items.as_slice() {
        [var, n] => {
            let sym = symbol_id(vm, *var)?;
            let n = number_of(vm, *n)?.as_exact_integer()?;
            Some((Some(sym), crate::execution::builtins::catalog::range_ints(vm, 1, n, 1)?))
        }
        [var, a, b] => {
            let sym = symbol_id(vm, *var)?;
            let a = number_of(vm, *a)?.as_exact_integer()?;
            let b = number_of(vm, *b)?.as_exact_integer()?;
            Some((Some(sym), crate::execution::builtins::catalog::range_ints(vm, a, b, 1)?))
        }
        [var, a, b, step] => {
            let sym = symbol_id(vm, *var)?;
            let a = number_of(vm, *a)?.as_exact_integer()?;
            let b = number_of(vm, *b)?.as_exact_integer()?;
            let step = number_of(vm, *step)?.as_exact_integer()?;
            Some((Some(sym), crate::execution::builtins::catalog::range_ints(vm, a, b, step)?))
        }
        [n] => {
            let n = number_of(vm, *n)?.as_exact_integer()?;
            Some((None, crate::execution::builtins::catalog::range_ints(vm, 1, n, 1)?))
        }
        _ => None,
    }
}

fn symbol_id(vm: &Vm<'_>, id: ExprId) -> Option<athena_types::SymbolId> {
    match vm.session.arena.get(id) {
        Some(ExprNode::Atom(Atom::Symbol(s))) => Some(*s),
        _ => None,
    }
}

pub(crate) fn h_sum(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    sum_product(vm, "Sum", args)
}

pub(crate) fn h_product(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    sum_product(vm, "Product", args)
}

/// `Sum[list]` → 数组求和；`Sum[body, iterator]` → 符号求和折叠。
pub(crate) fn sum_product(vm: &mut Vm<'_>, head: &str, args: &[ExprId]) -> Outcome {
    match args {
        [only] if head == "Sum" => {
            let o = vm.eval_value(*only);
            if o.has_error() {
                return o;
            }
            let summed = super::matrix::array_sum(vm, o.term);
            if summed.has_error() {
                return summed.with_diagnostics(o.diagnostics);
            }
            summed.with_diagnostics(o.diagnostics)
        }
        [_, _] => {
            let (table_o, values) = table(vm, args);
            if table_o.has_error() {
                return table_o;
            }
            let Some(values) = values
            else {
                let term = vm.push_app(head, vec![args[0], table_o.term]);
                return Outcome::unevaluated(term);
            };
            let folded = match head {
                "Sum" => {
                    if values.is_empty() {
                        vm.push_int(0)
                    }
                    else {
                        crate::execution::builtins::arithmetic::plus(vm, &values)
                    }
                }
                "Product" => {
                    if values.is_empty() {
                        vm.push_int(1)
                    }
                    else {
                        crate::execution::builtins::arithmetic::times(vm, &values)
                    }
                }
                _ => vm.push_app(head, args.to_vec()),
            };
            Outcome {
                term: folded,
                kind: crate::execution::EvalKind::Value,
                status: athena_types::ComputationStatus::Exact,
                diagnostics: table_o.diagnostics,
            }
        }
        _ => Outcome::unevaluated(vm.push_app(head, args.to_vec())),
    }
}

pub(crate) fn h_match_q(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app("MatchQ", args.to_vec()));
    }
    let expr_o = vm.eval_value(args[0]);
    // 模式参数保持 Hold-ish：不求值 Blank/Pattern。
    let matched = crate::execution::builtins::patterns::pattern_matches(vm, expr_o.term, args[1]);
    let term = vm.push_bool(matched);
    Outcome {
        term,
        kind: crate::execution::EvalKind::Value,
        status: athena_types::ComputationStatus::Exact,
        diagnostics: expr_o.diagnostics,
    }
}

pub(crate) fn h_cases(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    if args.len() != 2 {
        return Outcome::unevaluated(vm.push_app("Cases", args.to_vec()));
    }
    let list_o = vm.eval_value(args[0]);
    let Some(items) = (match vm.session.arena.get(list_o.term) {
        Some(ExprNode::List(_)) => vm.app_args(list_o.term),
        _ => None,
    })
    else {
        let term = vm.push_app("Cases", vec![list_o.term, args[1]]);
        return Outcome::unevaluated(term).with_diagnostics(list_o.diagnostics);
    };
    let out: Vec<ExprId> =
        items.into_iter().filter(|item| crate::execution::builtins::patterns::pattern_matches(vm, *item, args[1])).collect();
    Outcome {
        term: vm.push_list(out),
        kind: crate::execution::EvalKind::Value,
        status: athena_types::ComputationStatus::Exact,
        diagnostics: list_o.diagnostics,
    }
}

/// `Function` 未应用：惰性重建（占位保持 ids::FUNCTION_REBUILD 对齐）。
pub(crate) fn h_function_rebuild(vm: &mut Vm<'_>, args: &[ExprId]) -> Outcome {
    let _ = (non_boolean_condition_diagnostic, expression_summary);
    Outcome::unevaluated(vm.push_app("Function", args.to_vec()))
}
