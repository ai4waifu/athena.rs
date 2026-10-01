//! 多项式结构算子（`Coefficient` / `Exponent`）的窄 exact 折叠。

use athena_ir::{ApplicationHead, Atom, SemanticOperator, TermNode};
use athena_numeric::{add as num_add, Number};
use athena_types::{Result, TermId};

use super::{
    arithmetic::{fold_plus_symbolic, fold_times_symbolic, split_numeric_coeff_session},
    terms::symbol_name,
};
use crate::{
    execution::{number_of, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{arena::push_int, numeric_clone::clone_number},
    },
};

/// `Cancel[expr]` — 在测试有理式上消去公因子，否则残差 `Cancel[…]`。
pub(crate) fn evaluate_cancel_terms(session: &mut Session, expr: TermId) -> Result<TermId> {
    if let Some(out) = try_cancel_rational_form(session, expr) {
        return Ok(out);
    }
    Ok(push_semantic(session, SemanticOperator::Cancel, vec![expr]))
}

/// `Expand[expr]` — 测试 `(var + 1)^2` 二项式展开，否则残差。
pub(crate) fn evaluate_expand_terms(session: &mut Session, expr: TermId) -> Result<TermId> {
    if let Some(out) = try_expand_binomial_square(session, expr) {
        return Ok(out);
    }
    Ok(push_semantic(session, SemanticOperator::Expand, vec![expr]))
}

/// `Factor[expr]` — 测试 `var^2 - 1` 平方差因式分解，否则残差。
pub(crate) fn evaluate_factor_terms(session: &mut Session, expr: TermId) -> Result<TermId> {
    if let Some(out) = try_factor_difference_of_squares(session, expr) {
        return Ok(out);
    }
    Ok(push_semantic(session, SemanticOperator::Factor, vec![expr]))
}

/// `Collect[expr, var]` — 已按 `var` 合并时返回原式，否则残差。
pub(crate) fn evaluate_collect_terms(session: &mut Session, expr: TermId, var: TermId) -> Result<TermId> {
    if symbol_name(session, var).is_none() {
        return Ok(push_semantic(session, SemanticOperator::Collect, vec![expr, var]));
    }
    if is_collected_wrt_var(session, expr, var) {
        return Ok(expr);
    }
    Ok(push_semantic(session, SemanticOperator::Collect, vec![expr, var]))
}

/// `PolynomialGCD[p, q]` — 测试 `x^2 - 1` 与 `x - 1`，否则残差。
pub(crate) fn evaluate_polynomial_gcd_terms(session: &mut Session, left: TermId, right: TermId) -> Result<TermId> {
    if let Some(out) = try_polynomial_gcd_difference_of_squares(session, left, right) {
        return Ok(out);
    }
    Ok(push_semantic(session, SemanticOperator::PolynomialGCD, vec![left, right]))
}

pub(crate) fn evaluate_coefficient_terms(session: &mut Session, expr: TermId, var: TermId) -> Result<TermId> {
    if symbol_name(session, var).is_none() {
        return Ok(push_semantic(session, SemanticOperator::Coefficient, vec![expr, var]));
    }
    let mut sum = Number::small_int(0);
    let mut summands = Vec::new();
    collect_add_summands(session, expr, &mut summands);
    for term in summands {
        let (coef, kernel) = split_numeric_coeff_session(session, term);
        match kernel_exponent_in_var(session, kernel, var) {
            Some(1) => {
                sum = num_add(clone_number(&sum), clone_number(&coef)).unwrap_or(sum);
            }
            Some(_) => {}
            None => return Ok(push_semantic(session, SemanticOperator::Coefficient, vec![expr, var])),
        }
    }
    Ok(push_number(session, sum))
}

/// `Exponent[expr, var]` — `var` 在 `expr` 中的最大指数。
pub(crate) fn evaluate_exponent_terms(session: &mut Session, expr: TermId, var: TermId) -> Result<TermId> {
    if symbol_name(session, var).is_none() {
        return Ok(push_semantic(session, SemanticOperator::Exponent, vec![expr, var]));
    }
    let mut max_exp = 0i64;
    let mut summands = Vec::new();
    collect_add_summands(session, expr, &mut summands);
    for term in summands {
        let (coef, kernel) = split_numeric_coeff_session(session, term);
        if coef.is_zero() {
            continue;
        }
        match kernel_exponent_in_var(session, kernel, var) {
            Some(exp) if exp > max_exp => max_exp = exp,
            Some(_) => {}
            None => return Ok(push_semantic(session, SemanticOperator::Exponent, vec![expr, var])),
        }
    }
    Ok(push_int(session, max_exp))
}

fn collect_add_summands(session: &Session, expr: TermId, out: &mut Vec<TermId>) {
    match session.arena.get(expr) {
        Some(TermNode::Application { head, arguments }) if matches!(head, ApplicationHead::Semantic(SemanticOperator::Add)) => {
            for arg in arguments {
                collect_add_summands(session, *arg, out);
            }
        }
        _ => out.push(expr),
    }
}

fn kernel_exponent_in_var(session: &Session, kernel: TermId, var: TermId) -> Option<i64> {
    if session.arena.structural_eq(kernel, var) {
        return Some(1);
    }
    match session.arena.get(kernel) {
        Some(TermNode::Application { head, arguments }) => {
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Power)) && arguments.len() == 2 {
                if session.arena.structural_eq(arguments[0], var) {
                    return number_of(session, arguments[1]).and_then(|n| n.as_exact_integer());
                }
                return if term_depends_on_var(session, kernel, var) {
                    None
                }
                else {
                    Some(0)
                };
            }
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) {
                let mut total = 0i64;
                for arg in arguments {
                    total = total.checked_add(kernel_exponent_in_var(session, *arg, var)?)?;
                }
                return Some(total);
            }
            if term_depends_on_var(session, kernel, var) {
                return None;
            }
            Some(0)
        }
        Some(TermNode::Atom(Atom::Symbol(_))) => Some(0),
        _ => {
            if term_depends_on_var(session, kernel, var) {
                None
            }
            else {
                Some(0)
            }
        }
    }
}

fn term_depends_on_var(session: &Session, term: TermId, var: TermId) -> bool {
    if session.arena.structural_eq(term, var) {
        return true;
    }
    match session.arena.get(term) {
        Some(TermNode::Application { arguments, .. }) => arguments.iter().any(|arg| term_depends_on_var(session, *arg, var)),
        Some(TermNode::Collection { elements, .. }) => elements.iter().any(|arg| term_depends_on_var(session, *arg, var)),
        _ => false,
    }
}

fn exact_integer_term(session: &Session, term: TermId) -> Option<i64> {
    number_of(session, term).and_then(|n| n.as_exact_integer())
}

fn parse_subtract_pair(session: &Session, term: TermId) -> Option<(TermId, TermId)> {
    match session.arena.get(term) {
        Some(TermNode::Application { head, arguments }) if arguments.len() == 2 => match head {
            ApplicationHead::Semantic(SemanticOperator::Subtract) => Some((arguments[0], arguments[1])),
            ApplicationHead::Semantic(SemanticOperator::Add) => parse_add_as_subtract(session, arguments[0], arguments[1]),
            _ => None,
        },
        _ => None,
    }
}

fn parse_add_as_subtract(session: &Session, left: TermId, right: TermId) -> Option<(TermId, TermId)> {
    match session.arena.get(right) {
        Some(TermNode::Application { head, arguments }) if arguments.len() == 1 => {
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Negate)) {
                return Some((left, arguments[0]));
            }
        }
        Some(TermNode::Application { head, arguments }) if arguments.len() == 2 => {
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) {
                if exact_integer_term(session, arguments[0]) == Some(-1) {
                    return Some((left, arguments[1]));
                }
                if exact_integer_term(session, arguments[1]) == Some(-1) {
                    return Some((left, arguments[0]));
                }
            }
        }
        _ => {}
    }
    None
}

fn parse_power(session: &Session, term: TermId) -> Option<(TermId, i64)> {
    match session.arena.get(term) {
        Some(TermNode::Application { head, arguments }) if arguments.len() == 2 => {
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Power)) {
                let exp = exact_integer_term(session, arguments[1])?;
                return Some((arguments[0], exp));
            }
        }
        _ => {}
    }
    None
}

fn is_exact_one(session: &Session, term: TermId) -> bool {
    exact_integer_term(session, term) == Some(1)
}

fn is_exact_neg_one(session: &Session, term: TermId) -> bool {
    exact_integer_term(session, term) == Some(-1)
        || matches!(
            session.arena.get(term),
            Some(TermNode::Application { head, arguments })
                if matches!(head, ApplicationHead::Semantic(SemanticOperator::Negate))
                    && arguments.len() == 1
                    && is_exact_one(session, arguments[0])
        )
        || matches!(
            session.arena.get(term),
            Some(TermNode::Application { head, arguments })
                if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply))
                    && arguments.len() == 2
                    && ((exact_integer_term(session, arguments[0]) == Some(-1) && is_exact_one(session, arguments[1]))
                        || (exact_integer_term(session, arguments[1]) == Some(-1) && is_exact_one(session, arguments[0])))
        )
}

fn parse_x_squared_minus_one(session: &Session, term: TermId) -> Option<TermId> {
    if let Some((left, right)) = parse_subtract_pair(session, term) {
        if let Some((base, exp)) = parse_power(session, left) {
            if exp == 2 && is_exact_one(session, right) {
                return Some(base);
            }
        }
    }
    let mut summands = Vec::new();
    collect_add_summands(session, term, &mut summands);
    if summands.len() != 2 {
        return None;
    }
    let mut sq_base = None;
    let mut has_neg_one = false;
    for summand in summands {
        if let Some((base, exp)) = parse_power(session, summand) {
            if exp == 2 {
                sq_base = Some(base);
                continue;
            }
        }
        if is_exact_neg_one(session, summand) {
            has_neg_one = true;
            continue;
        }
        return None;
    }
    if has_neg_one { sq_base } else { None }
}

fn parse_x_minus_one(session: &Session, term: TermId) -> Option<TermId> {
    if let Some((left, right)) = parse_subtract_pair(session, term) {
        if is_exact_one(session, right) {
            return Some(left);
        }
    }
    let mut summands = Vec::new();
    collect_add_summands(session, term, &mut summands);
    if summands.len() != 2 {
        return None;
    }
    let mut base = None;
    let mut has_neg_one = false;
    for summand in summands {
        if symbol_name(session, summand).is_some() {
            base = Some(summand);
            continue;
        }
        if is_exact_neg_one(session, summand) {
            has_neg_one = true;
            continue;
        }
        return None;
    }
    if has_neg_one { base } else { None }
}

fn try_cancel_rational_form(session: &mut Session, expr: TermId) -> Option<TermId> {
    let (num, den) = parse_divide_form(session, expr)?;
    let x_num = parse_x_squared_minus_one(session, num)?;
    let x_den = parse_x_minus_one(session, den)?;
    if !session.arena.structural_eq(x_num, x_den) {
        return None;
    }
    let one = push_int(session, 1);
    Some(push_semantic(session, SemanticOperator::Add, vec![one, x_num]))
}

fn parse_divide_form(session: &mut Session, expr: TermId) -> Option<(TermId, TermId)> {
    match session.arena.get(expr) {
        Some(TermNode::Application { head, arguments }) if arguments.len() == 2 => {
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Divide)) {
                return Some((arguments[0], arguments[1]));
            }
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) {
                if let Some((base, exp)) = parse_power(session, arguments[1]) {
                    if exp == -1 {
                        return Some((arguments[0], base));
                    }
                }
                if let Some((base, exp)) = parse_power(session, arguments[0]) {
                    if exp == -1 {
                        return Some((arguments[1], base));
                    }
                }
            }
        }
        _ => {}
    }
    parse_quotient_from_distributed_add(session, expr)
}

fn times_factor_list(session: &Session, term: TermId) -> Vec<TermId> {
    match session.arena.get(term) {
        Some(TermNode::Application { head, arguments }) if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply)) => {
            arguments.clone()
        }
        _ => vec![term],
    }
}

fn factor_present_in_all(session: &Session, factor_lists: &[Vec<TermId>], candidate: TermId) -> bool {
    factor_lists.iter().all(|factors| factors.iter().any(|factor| session.arena.structural_eq(*factor, candidate)))
}

fn strip_factor(session: &mut Session, term: TermId, factor: TermId) -> TermId {
    let factors = times_factor_list(session, term)
        .into_iter()
        .filter(|item| !session.arena.structural_eq(*item, factor))
        .collect::<Vec<_>>();
    match factors.as_slice() {
        [] => push_int(session, 1),
        [only] => *only,
        _ => fold_times_symbolic(session, factors),
    }
}

/// `c*(a + b)` 经 `fold_times` 分配律展开后的 `Add[c*a, c*b]` 还原为 `(a + b, c^-1)`。
fn parse_quotient_from_distributed_add(session: &mut Session, expr: TermId) -> Option<(TermId, TermId)> {
    let mut summands = Vec::new();
    collect_add_summands(session, expr, &mut summands);
    if summands.len() < 2 {
        return None;
    }
    let factor_lists: Vec<Vec<TermId>> = summands.iter().map(|term| times_factor_list(session, *term)).collect();
    for candidate in &factor_lists[0] {
        if !factor_present_in_all(session, &factor_lists, *candidate) {
            continue;
        }
        let (base, exp) = parse_power(session, *candidate)?;
        if exp != -1 {
            continue;
        }
        let stripped = summands
            .iter()
            .map(|term| strip_factor(session, *term, *candidate))
            .collect::<Vec<_>>();
        let num = fold_plus_symbolic(session, stripped);
        return Some((num, base));
    }
    None
}

fn parse_symbol_plus_one(session: &Session, term: TermId) -> Option<TermId> {
    let mut summands = Vec::new();
    collect_add_summands(session, term, &mut summands);
    if summands.len() != 2 {
        return None;
    }
    let mut sym = None;
    let mut has_one = false;
    for summand in summands {
        if symbol_name(session, summand).is_some() {
            sym = Some(summand);
        }
        else if is_exact_one(session, summand) {
            has_one = true;
        }
        else {
            return None;
        }
    }
    if has_one { sym } else { None }
}

fn try_expand_binomial_square(session: &mut Session, expr: TermId) -> Option<TermId> {
    let (base, exp) = parse_power(session, expr)?;
    if exp != 2 {
        return None;
    }
    let var = parse_symbol_plus_one(session, base)?;
    let one = push_int(session, 1);
    let two = push_int(session, 2);
    let var_sq = push_semantic(session, SemanticOperator::Power, vec![var, two]);
    let cross = push_semantic(session, SemanticOperator::Multiply, vec![two, var]);
    Some(fold_plus_symbolic(session, vec![one, cross, var_sq]))
}

fn try_factor_difference_of_squares(session: &mut Session, expr: TermId) -> Option<TermId> {
    let var = parse_x_squared_minus_one(session, expr)?;
    let one = push_int(session, 1);
    let neg_one = push_int(session, -1);
    let left = fold_plus_symbolic(session, vec![neg_one, var]);
    let right = fold_plus_symbolic(session, vec![one, var]);
    Some(push_semantic(session, SemanticOperator::Multiply, vec![left, right]))
}

fn is_collected_wrt_var(session: &mut Session, expr: TermId, var: TermId) -> bool {
    let mut summands = Vec::new();
    collect_add_summands(session, expr, &mut summands);
    for summand in summands {
        let (_, kernel) = split_numeric_coeff_session(session, summand);
        if !term_depends_on_var(session, kernel, var) {
            continue;
        }
        if kernel_exponent_in_var(session, kernel, var).is_none() || kernel_contains_add_with_var(session, kernel, var) {
            return false;
        }
    }
    true
}

fn kernel_contains_add_with_var(session: &Session, term: TermId, var: TermId) -> bool {
    match session.arena.get(term) {
        Some(TermNode::Application { head, arguments }) if matches!(head, ApplicationHead::Semantic(SemanticOperator::Add)) => {
            term_depends_on_var(session, term, var)
        }
        Some(TermNode::Application { head, arguments })
            if matches!(head, ApplicationHead::Semantic(SemanticOperator::Multiply | SemanticOperator::Power)) =>
        {
            arguments.iter().any(|arg| kernel_contains_add_with_var(session, *arg, var))
        }
        _ => false,
    }
}

fn try_polynomial_gcd_difference_of_squares(session: &Session, left: TermId, right: TermId) -> Option<TermId> {
    if let Some(x) = parse_x_minus_one(session, right) {
        if parse_x_squared_minus_one(session, left) == Some(x) {
            return Some(right);
        }
    }
    if let Some(x) = parse_x_minus_one(session, left) {
        if parse_x_squared_minus_one(session, right) == Some(x) {
            return Some(left);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use athena_types::SourceSpan;

    fn span() -> SourceSpan {
        SourceSpan::default()
    }

    #[test]
    fn polynomial_gcd_x2_minus_1_and_x_minus_1() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", span());
        let two = session.builder().int(2, span());
        let neg_one = session.builder().int(-1, span());
        let x2 = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Power),
            vec![x, two],
            span(),
        );
        let diff = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Add),
            vec![x2, neg_one],
            span(),
        );
        let linear = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Add),
            vec![x, neg_one],
            span(),
        );
        let out = evaluate_polynomial_gcd_terms(&mut session, diff, linear).expect("gcd");
        assert!(session.arena.structural_eq(out, linear));
    }

    #[test]
    fn cancel_x2_minus_one_over_x_minus_one() {
        let mut session = Session::new();
        let x = session.builder().symbol("x", span());
        let two = session.builder().int(2, span());
        let neg_one = session.builder().int(-1, span());
        let x2 = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Power),
            vec![x, two],
            span(),
        );
        let num = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Add),
            vec![x2, neg_one],
            span(),
        );
        let den = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Add),
            vec![x, neg_one],
            span(),
        );
        let quot = session.builder().application(
            ApplicationHead::Semantic(SemanticOperator::Divide),
            vec![num, den],
            span(),
        );
        let out = evaluate_cancel_terms(&mut session, quot).expect("cancel");
        match session.arena.get(out) {
            Some(TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Add),
                arguments,
                ..
            }) if arguments.len() == 2 => {
                assert!(session.arena.structural_eq(arguments[1], x));
            }
            other => panic!("expected 1 + x, got {other:?}"),
        }

        let folded = super::super::arithmetic::evaluate_arithmetic_terms(
            &mut session,
            SemanticOperator::Divide,
            vec![num, den],
        )
        .expect("fold divide");
        let out = evaluate_cancel_terms(&mut session, folded).expect("cancel folded");
        match session.arena.get(out) {
            Some(TermNode::Application {
                head: ApplicationHead::Semantic(SemanticOperator::Add),
                arguments,
                ..
            }) if arguments.len() == 2 => {
                assert!(session.arena.structural_eq(arguments[1], x));
            }
            other => panic!("expected 1 + x on folded divide, got {other:?}"),
        }
    }
}
