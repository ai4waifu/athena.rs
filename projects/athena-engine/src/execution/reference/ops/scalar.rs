//! 标量 / 重写 / pattern 算子求值。

use std::collections::HashMap;

use athena_ir::{ApplicationHead, SemanticOperator};
use athena_vm::SlotTable;
use athena_types::Result;

use super::super::{ReferenceExecutor, Slot, helpers::*};
use crate::{
    api::request::AthenaRequest,
    execution::{compiler::ExecutionCompiler, ir::SsaValueId, push_extension, push_semantic},
    runtime::session::Session,
};

impl ReferenceExecutor {
    pub(crate) fn eval_residual_app(
        &self,
        session: &mut Session,
        op: athena_types::ExtensionOperatorId,
        args: &[SsaValueId],
        slots: &SlotTable,
    ) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        Ok(Slot::Term(push_extension(session, op, terms)))
    }

    pub(crate) fn eval_residual_semantic(
        &self,
        session: &mut Session,
        op: SemanticOperator,
        args: &[SsaValueId],
        slots: &SlotTable,
    ) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        if op.as_unary().is_some() {
            return Ok(Slot::Term(evaluate_special_unary_terms(session, op, terms)?));
        }
        Ok(Slot::Term(push_semantic(session, op, terms)))
    }

    /// 应用首条匹配的 Session 分派规则并重新求值替换式。
    pub(crate) fn try_apply_down_values(
        &self,
        session: &mut Session,
        op: athena_types::ExtensionOperatorId,
        args: &[SsaValueId],
        slots: &SlotTable,
    ) -> Result<Option<Slot>> {
        let Some(rules) = session
            .defs
            .extension_dispatch_rules(op)
            .map(|r| r.iter().map(|(pattern, replacement)| (pattern.owning_copy(), *replacement)).collect::<Vec<_>>())
        else {
            return Ok(None);
        };
        let call_op = ApplicationHead::Extension(op);
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let substituted = {
            let mut matched = None;
            for (pattern, rhs) in rules {
                let mut binds = HashMap::new();
                let ok = match &pattern {
                    crate::reasoning::trs::TermPattern::Application { operator, arguments } => {
                        *operator == call_op && arguments.len() == terms.len() && {
                            arguments
                                .iter()
                                .zip(terms.iter())
                                .all(|(p, a)| crate::execution::builtins::patterns::match_term_pattern(session, *a, p, &mut binds))
                        }
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
            matched
        };
        let Some(substituted) = substituted
        else {
            return Ok(None);
        };
        let module = ExecutionCompiler::new().compile(session, &AthenaRequest::Term(substituted))?;
        let result_id = self.execute(session, &module, None)?;
        let out = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(substituted);
        Ok(Some(Slot::Term(out)))
    }
}
