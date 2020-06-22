//! 扩展算子 down-value 分派。

use std::collections::HashMap;

use athena_ir::ApplicationHead;
use athena_types::{ExtensionOperatorId, Result, TermId};

use super::re_eval_term;
use crate::{
    execution::push_extension,
    runtime::session::Session,
};

/// 尝试 Session 扩展规则；命中则替换并再求值，否则 `None`。
pub(crate) fn try_apply_extension_down_values(
    session: &mut Session,
    op: ExtensionOperatorId,
    terms: &[TermId],
) -> Result<Option<TermId>> {
    let Some(rules) = session
        .defs
        .extension_dispatch_rules(op)
        .map(|r| r.iter().map(|(pattern, replacement)| (pattern.owning_copy(), *replacement)).collect::<Vec<_>>())
    else {
        return Ok(None);
    };
    let call_op = ApplicationHead::Extension(op);
    let mut matched = None;
    for (pattern, rhs) in rules {
        let mut binds = HashMap::new();
        let ok = match &pattern {
            crate::reasoning::trs::TermPattern::Application { operator, arguments } => {
                *operator == call_op
                    && arguments.len() == terms.len()
                    && arguments
                        .iter()
                        .zip(terms.iter())
                        .all(|(p, a)| crate::execution::builtins::patterns::match_term_pattern(session, *a, p, &mut binds))
            }
            crate::reasoning::trs::TermPattern::StructuralApplication(arguments) => {
                arguments.len() == terms.len()
                    && arguments
                        .iter()
                        .zip(terms.iter())
                        .all(|(p, a)| crate::execution::builtins::patterns::match_term_pattern(session, *a, p, &mut binds))
            }
            _ => false,
        };
        if ok {
            matched = Some(crate::execution::builtins::patterns::substitute_binds(session, rhs, &binds));
            break;
        }
    }
    let Some(substituted) = matched else {
        return Ok(None);
    };
    Ok(Some(re_eval_term(session, substituted)?))
}

/// `Extension[args…]` — down-value 或残差扩展应用。
pub(crate) fn evaluate_extension_apply_terms(
    session: &mut Session,
    op: ExtensionOperatorId,
    terms: Vec<TermId>,
) -> Result<(TermId, bool)> {
    if let Some(term) = try_apply_extension_down_values(session, op, &terms)? {
        return Ok((term, false));
    }
    Ok((push_extension(session, op, terms), true))
}
