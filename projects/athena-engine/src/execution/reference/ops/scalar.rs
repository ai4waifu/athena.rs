//! 标量残差（语义未知形态）。

use athena_ir::SemanticOperator;
use athena_vm::SlotTable;
use athena_types::Result;

use super::super::{ReferenceExecutor, Slot, helpers::*};
use crate::{
    execution::{ir::SsaValueId, push_semantic},
    runtime::session::Session,
};

impl ReferenceExecutor {
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
}
