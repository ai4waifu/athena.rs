//! Part / Span / Apply / ReplaceAll / Map handler（legacy 对应 `eval_*` 语义）。

use athena_ir::{Atom, TermNode};
use athena_types::TermId;

use crate::execution::{
    Outcome,
    builtins::{arithmetic::number_of, catalog::invalid_index_diagnostic},
    vm::Vm,
};

pub(crate) fn h_span(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let mut ints = Vec::with_capacity(args.len());
    for a in args {
        match number_of(vm, *a).and_then(|n| n.as_exact_integer()) {
            Some(v) => ints.push(v),
            None => return Outcome::unevaluated(vm.push_application("Span", args.to_vec())),
        }
    }
    let list = match ints.as_slice() {
        [a, b] => expand_span_2(vm, *a, *b),
        [a, step, b] => expand_span_3(vm, *a, *step, *b),
        _ => None,
    };
    match list {
        Some(items) => Outcome::value(vm.push_list(items)),
        None => Outcome::unevaluated(vm.push_application("Span", args.to_vec())),
    }
}

fn expand_span_2(vm: &mut Vm<'_>, a: i64, b: i64) -> Option<Vec<TermId>> {
    let mut out = Vec::new();
    if a <= b {
        let mut x = a;
        while x <= b {
            out.push(vm.push_int(x));
            x += 1;
        }
    }
    else {
        let mut x = a;
        while x >= b {
            out.push(vm.push_int(x));
            x -= 1;
        }
    }
    Some(out)
}

fn expand_span_3(vm: &mut Vm<'_>, a: i64, step: i64, b: i64) -> Option<Vec<TermId>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut x = a;
    if step > 0 {
        while x <= b {
            out.push(vm.push_int(x));
            x += step;
        }
    }
    else {
        while x >= b {
            out.push(vm.push_int(x));
            x += step;
        }
    }
    Some(out)
}

pub(crate) fn h_part(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    part_n(vm, args)
}

/// `Part[m, All, j, …]` — 剩余下标映射到各行（MATLAB `A(:,j)`）。
fn part_n(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    if args.len() < 2 {
        return Outcome::unevaluated(vm.push_application("Part", args.to_vec()));
    }
    if is_all_symbol(vm, args[1]) && args.len() >= 3 && matches!(vm.session.arena.get(args[0]), Some(TermNode::List(_))) {
        let rows = vm.application_arguments(args[0]).unwrap_or_default();
        let mut diags = Vec::new();
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let mut part_args = Vec::with_capacity(args.len() - 1);
            part_args.push(row);
            part_args.extend_from_slice(&args[2..]);
            let o = part_n(vm, &part_args);
            let errored = o.has_error();
            diags.extend(o.diagnostics);
            if errored {
                return Outcome { term: o.term, kind: o.kind, status: o.status, diagnostics: diags };
            }
            out.push(o.term);
        }
        return Outcome {
            term: vm.push_list(out),
            kind: crate::execution::EvalKind::Value,
            status: athena_types::ComputationStatus::Exact,
            diagnostics: diags,
        };
    }
    let mut cur = args[0];
    let mut diags = Vec::new();
    for index in &args[1..] {
        let o = part_outcome(vm, cur, *index);
        let errored = o.has_error();
        diags.extend(o.diagnostics);
        if errored {
            return Outcome { term: o.term, kind: o.kind, status: o.status, diagnostics: diags };
        }
        cur = o.term;
    }
    Outcome {
        term: cur,
        kind: crate::execution::EvalKind::Value,
        status: athena_types::ComputationStatus::Exact,
        diagnostics: diags,
    }
}

fn is_end_symbol(vm: &Vm<'_>, term: TermId) -> bool {
    match vm.session.arena.get(term) {
        Some(TermNode::Atom(Atom::Symbol(s))) => {
            matches!(vm.session.arena.symbols().resolve(*s), Some("End") | Some("end"))
        }
        _ => false,
    }
}

fn is_all_symbol(vm: &Vm<'_>, term: TermId) -> bool {
    match vm.session.arena.get(term) {
        Some(TermNode::Atom(Atom::Symbol(s))) => {
            matches!(vm.session.arena.symbols().resolve(*s), Some("All") | Some(":"))
        }
        _ => false,
    }
}

fn part_outcome(vm: &mut Vm<'_>, expr: TermId, index: TermId) -> Outcome {
    // 索引列表：逐个抽取再组列表。
    if matches!(vm.session.arena.get(index), Some(TermNode::List(_))) {
        let indices = vm.application_arguments(index).unwrap_or_default();
        let mut diags = Vec::new();
        let mut out = Vec::with_capacity(indices.len());
        for idx in indices {
            let o = part_outcome(vm, expr, idx);
            let errored = o.has_error();
            diags.extend(o.diagnostics);
            if errored {
                return Outcome { term: o.term, kind: o.kind, status: o.status, diagnostics: diags };
            }
            out.push(o.term);
        }
        return Outcome {
            term: vm.push_list(out),
            kind: crate::execution::EvalKind::Value,
            status: athena_types::ComputationStatus::Exact,
            diagnostics: diags,
        };
    }

    if matches!(vm.session.arena.get(expr), Some(TermNode::List(_))) {
        let items = vm.application_arguments(expr).unwrap_or_default();
        if is_end_symbol(vm, index) {
            let end = vm.push_int(items.len() as i64);
            return part_outcome(vm, expr, end);
        }
        if is_all_symbol(vm, index) {
            return Outcome::value(vm.push_list(items));
        }
        let len = items.len();
        let Some(idx) = number_of(vm, index).and_then(|n| n.as_exact_integer())
        else {
            return Outcome::unevaluated(vm.push_application("Part", vec![expr, index]));
        };
        if idx == 0 {
            // Mathematica：`Part[list, 0]` 为 head `List`。
            return Outcome::value(vm.push_symbol("List"));
        }
        let i = if idx > 0 {
            (idx - 1) as usize
        }
        else {
            let n = len as i64;
            let pos = n + idx;
            if pos < 0 || pos as usize >= len {
                let echo = vm.push_application("Part", vec![expr, index]);
                return Outcome::invalid(echo, invalid_index_diagnostic(idx, Some(len as u64)));
            }
            pos as usize
        };
        return match items.get(i) {
            Some(item) => Outcome::value(*item),
            None => {
                let echo = vm.push_application("Part", vec![expr, index]);
                Outcome::invalid(echo, invalid_index_diagnostic(idx, Some(len as u64)))
            }
        };
    }
    Outcome::unevaluated(vm.push_application("Part", vec![expr, index]))
}

pub(crate) fn h_apply(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    if matches!(vm.session.arena.get(args[1]), Some(TermNode::List(_))) {
        let items = vm.application_arguments(args[1]).unwrap_or_default();
        let app = vm.rebuild_application(args[0], items);
        return vm.eval_value(app);
    }
    Outcome::unevaluated(vm.push_application("Apply", vec![args[0], args[1]]))
}

pub(crate) fn h_replace_all(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let rule_list: Vec<(TermId, TermId)> = match vm.session.arena.get(args[1]) {
        Some(TermNode::List(items)) => items.iter().filter_map(|r| rule_pair(vm, *r)).collect(),
        _ => rule_pair(vm, args[1]).into_iter().collect(),
    };
    let mut cur = args[0];
    for (lhs, rhs) in rule_list {
        cur = crate::execution::builtins::patterns::replace_literal(vm, cur, lhs, rhs);
    }
    let o = vm.eval_value(cur);
    o
}

fn rule_pair(vm: &Vm<'_>, expr: TermId) -> Option<(TermId, TermId)> {
    let TermNode::Application { arguments: args, .. } = vm.session.arena.get(expr)?
    else {
        return None;
    };
    if args.len() == 2 && matches!(vm.head_name(expr).as_deref(), Some("Rule") | Some("RuleDelayed")) {
        Some((args[0], args[1]))
    }
    else {
        None
    }
}

pub(crate) fn h_map(vm: &mut Vm<'_>, args: &[TermId]) -> Outcome {
    let Some(items) = (match vm.session.arena.get(args[1]) {
        Some(TermNode::List(_)) => vm.application_arguments(args[1]),
        _ => None,
    })
    else {
        return Outcome::value(vm.push_application("Map", vec![args[0], args[1]]));
    };
    let mut out = Vec::with_capacity(items.len());
    let mut diags = Vec::new();
    for item in items {
        let mapped = map_one(vm, args[0], item);
        let o = vm.eval_value(mapped);
        diags.extend(o.diagnostics);
        out.push(o.term);
    }
    Outcome::value(vm.push_list(out)).with_diagnostics(diags)
}

fn map_one(vm: &mut Vm<'_>, func: TermId, item: TermId) -> TermId {
    if let Some(name) = symbol_name(vm, func) {
        return vm.push_application(&name, vec![item]);
    }
    if is_function_arity(vm, func, 1) {
        let body = vm.application_arguments(func).unwrap_or_default()[0];
        return crate::execution::builtins::patterns::substitute_slot(vm, body, item);
    }
    if is_function_arity(vm, func, 2) {
        let fargs = vm.application_arguments(func).unwrap_or_default();
        if let Some(var) = symbol_name(vm, fargs[0]) {
            let sym = vm.session.arena.symbols_mut().intern(var);
            return crate::execution::builtins::patterns::substitute_symbol(vm, fargs[1], sym, item);
        }
    }
    vm.push_application("Map", vec![func, item])
}

fn symbol_name(vm: &Vm<'_>, id: TermId) -> Option<String> {
    match vm.session.arena.get(id) {
        Some(TermNode::Atom(Atom::Symbol(s))) => vm.session.arena.symbols().resolve(*s).map(str::to_string),
        _ => None,
    }
}

fn is_function_arity(vm: &Vm<'_>, id: TermId, arity: usize) -> bool {
    if vm.head_name(id).is_none_or(|h| h != "Function") {
        return false;
    }
    vm.application_arguments(id).is_some_and(|a| a.len() == arity)
}
