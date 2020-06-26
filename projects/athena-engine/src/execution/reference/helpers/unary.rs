//! 一元结构 / 数值算子求值（Reference 与 `ExecutionHost` 共用）。

use athena_ir::SemanticOperator;
use athena_numeric::{abs as num_abs, factorial as num_factorial, sqrt as num_sqrt};
use athena_types::{Result, TermId};

use super::diag;
use crate::{
    execution::{number_of, push_number, push_semantic},
    runtime::{
        session::Session,
        values::{arena::push_list, numeric_clone::clone_number},
    },
};

/// 一元 `Abs` / `Factorial` / `Sqrt` / `Length` / `First` / `Rest`。
pub(crate) fn evaluate_unary_term(session: &mut Session, op: SemanticOperator, term: TermId) -> Result<TermId> {
    match op {
        SemanticOperator::Abs => {
            if let Some(n) = number_of(session, term) {
                Ok(push_number(session, num_abs(clone_number(n))))
            }
            else {
                Ok(push_semantic(session, SemanticOperator::Abs, vec![term]))
            }
        }
        SemanticOperator::Factorial => {
            if let Some(n) = number_of(session, term) {
                match num_factorial(n) {
                    Ok(v) => Ok(push_number(session, v)),
                    Err(_) => Ok(push_semantic(session, SemanticOperator::Factorial, vec![term])),
                }
            }
            else {
                Ok(push_semantic(session, SemanticOperator::Factorial, vec![term]))
            }
        }
        SemanticOperator::Sqrt => {
            if let Some(n) = number_of(session, term) {
                match num_sqrt(n) {
                    Ok(Some(v)) => Ok(push_number(session, v)),
                    _ => Ok(push_semantic(session, SemanticOperator::Sqrt, vec![term])),
                }
            }
            else {
                Ok(push_semantic(session, SemanticOperator::Sqrt, vec![term]))
            }
        }
        SemanticOperator::Length => {
            let len = match session.arena.get(term) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.len() as i64,
                Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.len() as i64,
                _ => return Ok(push_semantic(session, SemanticOperator::Length, vec![term])),
            };
            Ok(session.builder().int(len, Default::default()))
        }
        SemanticOperator::First => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => Ok(items[0]),
            Some(athena_ir::TermNode::Application { arguments, .. }) if !arguments.is_empty() => Ok(arguments[0]),
            Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("first_empty")),
            _ => Ok(push_semantic(session, SemanticOperator::First, vec![term])),
        },
        SemanticOperator::Rest => match session.arena.get(term) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) if !items.is_empty() => {
                let rest = items[1..].to_vec();
                Ok(push_list(session, rest))
            }
            Some(athena_ir::TermNode::Application { head, arguments }) if !arguments.is_empty() => {
                let head = *head;
                let rest = arguments[1..].to_vec();
                Ok(session.builder().application(head, rest, Default::default()))
            }
            Some(athena_ir::TermNode::Collection { elements: _, .. } | athena_ir::TermNode::Application { .. }) => Err(diag("rest_empty")),
            _ => Ok(push_semantic(session, SemanticOperator::Rest, vec![term])),
        },
        _ => Err(diag("semantic_operator_not_implemented")),
    }
}
