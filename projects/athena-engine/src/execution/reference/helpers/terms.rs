//! Term / matrix / iterator helpers for the reference executor.

use athena_numeric::{Integer, Number, Rational, to_f64_lossy as num_to_f64_lossy};
use athena_types::{Result, SymbolId, TermId};

use super::diag;
use athena_ir::{ApplicationHead, SemanticOperator};

use crate::{
    domains::linear_algebra::{MatrixEntry, MatrixValue},
    execution::{number_of, push_application, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{
            arena::push_list,
            numeric_clone::{clone_integer, clone_number, clone_rational},
        },
    },
};
use athena_ir::TermBuilder;

fn is_sem(head: ApplicationHead, op: SemanticOperator) -> bool {
    matches!(head, ApplicationHead::Semantic(o) if o == op)
}

fn head_label(session: &Session, head: ApplicationHead) -> Option<String> {
    match head {
        ApplicationHead::Semantic(op) => Some(op.debug_label().to_string()),
        ApplicationHead::Extension(id) => session.operators.name(id).map(str::to_string),
    }
}

pub(crate) fn eval_trig_exact_session(session: &mut Session, name: &str, arg: TermId) -> Option<TermId> {
    let angle = normalize_pi_angle_session(session, arg)?;
    match name {
        "Sin" => Some(session.builder().int(0, Default::default())),
        "Cos" => Some(session.builder().int(if angle % 2 == 0 { 1 } else { -1 }, Default::default())),
        "Tan" if angle % 2 == 0 => Some(session.builder().int(0, Default::default())),
        _ => None,
    }
}

pub(crate) fn term_as_f64_session(session: &Session, arg: TermId) -> Option<f64> {
    if let Some(k) = normalize_pi_angle_session(session, arg) {
        return Some((k as f64) * std::f64::consts::PI);
    }
    if head_name_session(session, arg).as_deref() == Some("E") {
        return Some(std::f64::consts::E);
    }
    number_of(session, arg).and_then(num_to_f64_lossy)
}

pub(crate) fn normalize_pi_angle_session(session: &Session, arg: TermId) -> Option<i64> {
    if let Some(n) = number_of(session, arg).and_then(|n| n.as_exact_integer()) {
        if n == 0 {
            return Some(0);
        }
    }
    if head_name_session(session, arg).as_deref() == Some("Pi") {
        return Some(1);
    }
    if let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(arg) {
        if is_sem(*head, SemanticOperator::Multiply) {
            if let [a, b] = arguments.as_slice() {
                if head_name_session(session, *a).as_deref() == Some("Pi") {
                    return number_of(session, *b).and_then(|n| n.as_exact_integer());
                }
                if head_name_session(session, *b).as_deref() == Some("Pi") {
                    return number_of(session, *a).and_then(|n| n.as_exact_integer());
                }
            }
        }
        if is_sem(*head, SemanticOperator::Add)
            && arguments.len() == 1
            && head_name_session(session, arguments[0]).as_deref() == Some("Pi")
        {
            return Some(1);
        }
    }
    None
}

pub(crate) fn head_name_session(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id)? {
        athena_ir::TermNode::Application { head, .. } => head_label(session, *head),
        athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol)) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

pub(crate) fn expand_span_2(a: i64, b: i64) -> Option<Vec<i64>> {
    let mut out = Vec::new();
    if a <= b {
        let mut x = a;
        while x <= b {
            out.push(x);
            x += 1;
        }
    }
    else {
        let mut x = a;
        while x >= b {
            out.push(x);
            x -= 1;
        }
    }
    Some(out)
}

pub(crate) fn expand_span_3(a: i64, step: i64, b: i64) -> Option<Vec<i64>> {
    if step == 0 {
        return None;
    }
    let mut out = Vec::new();
    let mut x = a;
    if step > 0 {
        while x <= b {
            out.push(x);
            x += step;
        }
    }
    else {
        while x >= b {
            out.push(x);
            x += step;
        }
    }
    Some(out)
}

/// Expand `{i,n}` / `{i,a,b}` / `{i,a,b,step}` / `{n}` for iterator `Sum`.
pub(crate) fn expand_iterator_session(session: &mut Session, spec: TermId) -> Option<(Option<SymbolId>, Vec<TermId>)> {
    let items = match session.arena.get(spec) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
        _ => return None,
    };
    match items.as_slice() {
        [var, n] => {
            let sym = term_symbol_id(session, *var)?;
            let n = number_of(session, *n)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, 1, n, 1)?))
        }
        [var, a, b] => {
            let sym = term_symbol_id(session, *var)?;
            let a = number_of(session, *a)?.as_exact_integer()?;
            let b = number_of(session, *b)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, a, b, 1)?))
        }
        [var, a, b, step] => {
            let sym = term_symbol_id(session, *var)?;
            let a = number_of(session, *a)?.as_exact_integer()?;
            let b = number_of(session, *b)?.as_exact_integer()?;
            let step = number_of(session, *step)?.as_exact_integer()?;
            Some((Some(sym), range_int_terms(session, a, b, step)?))
        }
        [n] => {
            let n = number_of(session, *n)?.as_exact_integer()?;
            Some((None, range_int_terms(session, 1, n, 1)?))
        }
        _ => None,
    }
}

pub(crate) fn term_symbol_id(session: &Session, id: TermId) -> Option<SymbolId> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(s))) => Some(*s),
        _ => None,
    }
}

pub(crate) fn range_int_terms(session: &mut Session, a: i64, b: i64, step: i64) -> Option<Vec<TermId>> {
    let ints = expand_span_3(a, step, b)?;
    Some(ints.into_iter().map(|n| session.builder().int(n, Default::default())).collect())
}

pub(crate) fn rebuild_application(session: &mut Session, head: TermId, args: Vec<TermId>) -> TermId {
    match session.arena.get(head) {
        // 0-ary semantic / extension application used as an operator value (not a Symbol name).
        Some(athena_ir::TermNode::Application {
            head: ApplicationHead::Semantic(op),
            arguments,
        }) if arguments.is_empty() => push_semantic(session, *op, args),
        Some(athena_ir::TermNode::Application {
            head: ApplicationHead::Extension(id),
            arguments,
        }) if arguments.is_empty() => {
            let id = *id;
            let mut b = TermBuilder::new(&mut session.arena);
            b.application_extension_id(id, args, athena_ir::TermNode::default_span())
        }
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => {
            let name = session.arena.symbols().resolve(*symbol).unwrap_or("?").to_string();
            push_application(session, &name, args)
        }
        _ => {
            let mut wrapped = Vec::with_capacity(args.len() + 1);
            wrapped.push(head);
            wrapped.extend(args);
            push_semantic(session, SemanticOperator::ApplyHead, wrapped)
        }
    }
}

pub(crate) fn symbol_name(session: &Session, id: TermId) -> Option<String> {
    match session.arena.get(id) {
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(symbol))) => session.arena.symbols().resolve(*symbol).map(str::to_string),
        _ => None,
    }
}

pub(crate) fn parse_matrix_dims(session: &Session, args: &[TermId]) -> Option<(u64, u64)> {
    let as_dim = |t: TermId| -> Option<u64> {
        let n = number_of(session, t)?.as_exact_integer()?;
        if n < 0 { None } else { Some(n as u64) }
    };
    match args {
        [n] => {
            let n = as_dim(*n)?;
            Some((n, n))
        }
        [m, n] => Some((as_dim(*m)?, as_dim(*n)?)),
        _ => None,
    }
}

pub(crate) fn collect_rule_pairs(session: &Session, rules_term: TermId) -> Vec<(TermId, TermId)> {
    match session.arena.get(rules_term) {
        Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.iter().filter_map(|r| rule_pair(session, *r)).collect(),
        _ => rule_pair(session, rules_term).into_iter().collect(),
    }
}

pub(crate) fn rule_pair(session: &Session, expr: TermId) -> Option<(TermId, TermId)> {
    let athena_ir::TermNode::Application { head, arguments } = session.arena.get(expr)?
    else {
        return None;
    };
    if arguments.len() != 2 {
        return None;
    }
    let name = head_label(session, *head)?;
    if matches!(name.as_str(), "Rule" | "RuleDeferred") { Some((arguments[0], arguments[1])) } else { None }
}

pub(crate) fn try_pythagorean_session(session: &mut Session, expr: TermId) -> Option<TermId> {
    let athena_ir::TermNode::Application { head, arguments } = session.arena.get(expr)?
    else {
        return None;
    };
    if !is_sem(*head, SemanticOperator::Add) || arguments.len() != 2 {
        return None;
    }
    let (a, b) = (arguments[0], arguments[1]);
    if is_trig_sq_session(session, a, "Sin") && is_trig_sq_session(session, b, "Cos") && same_trig_arg_session(session, a, b) {
        return Some(session.builder().int(1, Default::default()));
    }
    if is_trig_sq_session(session, a, "Cos") && is_trig_sq_session(session, b, "Sin") && same_trig_arg_session(session, a, b) {
        return Some(session.builder().int(1, Default::default()));
    }
    None
}

pub(crate) fn is_trig_sq_session(session: &Session, expr: TermId, name: &str) -> bool {
    let Some(athena_ir::TermNode::Application { head, arguments }) = session.arena.get(expr)
    else {
        return false;
    };
    if arguments.len() != 2 || !is_sem(*head, SemanticOperator::Power) {
        return false;
    }
    let exp_is_two = matches!(
        session.arena.get(arguments[1]),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer() == Some(2)
    );
    if !exp_is_two {
        return false;
    }
    match session.arena.get(arguments[0]) {
        Some(athena_ir::TermNode::Application { head, arguments: inner }) if inner.len() == 1 => head_label(session, *head).as_deref() == Some(name),
        _ => false,
    }
}

pub(crate) fn same_trig_arg_session(session: &Session, a: TermId, b: TermId) -> bool {
    let arg = |expr: TermId| -> Option<TermId> {
        let athena_ir::TermNode::Application { arguments, .. } = session.arena.get(expr)?
        else {
            return None;
        };
        if arguments.len() != 2 {
            return None;
        }
        let athena_ir::TermNode::Application { arguments: inner, .. } = session.arena.get(arguments[0])?
        else {
            return None;
        };
        (inner.len() == 1).then_some(inner[0])
    };
    match (arg(a), arg(b)) {
        (Some(x), Some(y)) => session.arena.structural_eq(x, y),
        _ => false,
    }
}

pub(crate) fn nested_list_shape(session: &Session, term: TermId) -> Option<(u64, u64)> {
    let athena_ir::TermNode::Collection { elements: rows, .. } = session.arena.get(term)?
    else {
        return None;
    };
    if rows.is_empty() {
        return Some((0, 0));
    }
    if matches!(session.arena.get(rows[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
        let mut cols: Option<u64> = None;
        for row in rows {
            let cells = match session.arena.get(*row) {
                Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.len() as u64,
                _ => return None,
            };
            match cols {
                Some(prev) if prev != cells => return None,
                None => cols = Some(cells),
                _ => {}
            }
        }
        Some((rows.len() as u64, cols.unwrap_or(0)))
    }
    else {
        Some((1, rows.len() as u64))
    }
}

pub(crate) fn term_scalar_rational_session(session: &Session, term: TermId) -> Option<Rational> {
    let n = number_of(session, term)?;
    if let Some(i) = n.as_exact_integer() {
        return Some(Rational::new(Integer::from_i64(i), Integer::one()));
    }
    if let Some(i) = n.as_integer() {
        return Some(Rational::new(clone_integer(i), Integer::one()));
    }
    n.as_rational().map(clone_rational)
}

pub(crate) fn term_to_rational_matrix_session(session: &Session, term: TermId) -> Option<MatrixValue> {
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Collection { elements: rows, .. }) if !rows.is_empty() => {
            if matches!(session.arena.get(rows[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
                let mut data = Vec::new();
                let mut cols: Option<u64> = None;
                for row in rows {
                    let cells = match session.arena.get(*row) {
                        Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.clone(),
                        _ => return None,
                    };
                    let c = cells.len() as u64;
                    match cols {
                        Some(prev) if prev != c => return None,
                        None => cols = Some(c),
                        _ => {}
                    }
                    for cell in cells {
                        data.push(term_scalar_rational_session(session, cell)?);
                    }
                }
                MatrixValue::from_rationals_row_major(rows.len() as u64, cols.unwrap_or(0), data).ok()
            }
            else {
                let mut data = Vec::with_capacity(rows.len());
                for cell in rows {
                    data.push(term_scalar_rational_session(session, *cell)?);
                }
                MatrixValue::from_rationals_row_major(1, data.len() as u64, data).ok()
            }
        }
        _ => {
            let r = term_scalar_rational_session(session, term)?;
            MatrixValue::from_rationals_row_major(1, 1, vec![r]).ok()
        }
    }
}

pub(crate) fn rational_to_term_session(session: &mut Session, r: &Rational) -> TermId {
    if r.is_integer() {
        if let Some(i) = r.numerator().to_i64() {
            return session.builder().int(i, Default::default());
        }
    }
    push_number(session, Number::from_rational_normalized(clone_rational(r)))
}

pub(crate) fn matrix_to_nested_list_session(session: &mut Session, m: &MatrixValue) -> Result<TermId> {
    let (rows, cols) = (m.shape().rows, m.shape().cols);
    let mut out = Vec::with_capacity(rows as usize);
    for i in 0..rows {
        let mut row = Vec::with_capacity(cols as usize);
        for j in 0..cols {
            match m.get(i, j)? {
                MatrixEntry::Rational(r) => row.push(rational_to_term_session(session, &r)),
                MatrixEntry::Integer(n) => {
                    if let Some(i64v) = n.to_i64() {
                        row.push(session.builder().int(i64v, Default::default()));
                    }
                    else {
                        row.push(push_number(session, Number::integer(clone_integer(&n))));
                    }
                }
                MatrixEntry::MachineF64(x) => row.push(push_number(session, Number::machine(x))),
            }
        }
        out.push(push_list(session, row));
    }
    Ok(push_list(session, out))
}
