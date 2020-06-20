//! 标量 / 重写 / pattern 算子求值。

use std::collections::HashMap;

use athena_ir::{ApplicationHead, SemanticOperator, UnaryFunction};
use athena_numeric::Number;
use athena_vm::SlotTable;
use athena_types::{Result, TermId};

use super::super::{ReferenceExecutor, Slot, helpers::*};
use crate::{
    api::request::AthenaRequest,
    execution::{compiler::ExecutionCompiler, ir::SsaValueId, number_of, push_extension, push_number, push_semantic},
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
        // 仅扩展残差 — 核心三角/特殊函数经 `SemanticOperator::Unary` 求值。
        Ok(Slot::Term(push_extension(session, op, terms)))
    }

    /// 应用首条匹配的 Session 分派规则并重新求值替换式。

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
        if let Some(uf) = op.as_unary() {
            if terms.len() == 1 {
                let arg = terms[0];
                if let Some(exact) = eval_trig_exact_session(session, uf, arg) {
                    return Ok(Slot::Term(exact));
                }
                // 仅当参数已是 machine 实数时折叠。禁止把精确 `Sin[1]` 经 `f64` 自动 `N`。
                if let Some(x) = number_of(session, arg).and_then(|n| n.as_machine_f64()) {
                    let y = match uf {
                        UnaryFunction::Sin => x.sin(),
                        UnaryFunction::Cos => x.cos(),
                        UnaryFunction::Tan => x.tan(),
                        UnaryFunction::Exp => x.exp(),
                        UnaryFunction::Log => x.ln(),
                        _ => f64::NAN,
                    };
                    if y.is_finite() {
                        return Ok(Slot::Term(push_number(session, Number::machine(y))));
                    }
                }
            }
        }
        Ok(Slot::Term(push_semantic(session, op, terms)))
    }

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
