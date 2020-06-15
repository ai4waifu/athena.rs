//! 列表 / 矩阵 / 算术算子求值。

use athena_ir::SemanticOperator;
use athena_vm::SlotTable;
use athena_types::{Result, TermId};

use super::super::{ReferenceExecutor, Slot, helpers::*};
use crate::{
    api::request::AthenaRequest,
    execution::{compiler::ExecutionCompiler, ir::SsaValueId, push_semantic},
    runtime::session::Session,
};

impl ReferenceExecutor {
    /// `Sum[list]` — 向量标量和 / 矩阵按列求和。
    /// `Sum[body, iterator]` — 展开迭代器再 Plus 折叠。
    pub(crate) fn eval_sum(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() == 2 {
            let body = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
            let iter = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
            return match self.table_values(session, body, iter)? {
                Some(values) => {
                    if values.is_empty() {
                        Ok(Slot::Term(session.builder().int(0, Default::default())))
                    }
                    else {
                        Ok(Slot::Term(fold_plus_symbolic(session, values)))
                    }
                }
                None => Ok(Slot::Term(push_semantic(session, SemanticOperator::Sum, vec![body, iter]))),
            };
        }
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        Ok(Slot::Term(evaluate_sum_terms(session, terms)?))
    }

    /// `Product[body, iterator]` — 展开迭代器再 Times 折叠。
    pub(crate) fn eval_product(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() == 2 {
            let body = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
            let iter = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
            return match self.table_values(session, body, iter)? {
                Some(values) => {
                    if values.is_empty() {
                        Ok(Slot::Term(session.builder().int(1, Default::default())))
                    }
                    else {
                        Ok(Slot::Term(fold_times_symbolic(session, values)))
                    }
                }
                None => Ok(Slot::Term(push_semantic(session, SemanticOperator::Product, vec![body, iter]))),
            };
        }
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        Ok(Slot::Term(push_semantic(session, SemanticOperator::Product, terms)))
    }

    pub(crate) fn table_values(&self, session: &mut Session, body: TermId, iter: TermId) -> Result<Option<Vec<TermId>>> {
        let Some((var, values)) = expand_iterator_session(session, iter)
        else {
            return Ok(None);
        };
        let mut out = Vec::with_capacity(values.len());
        for value in values {
            let instantiated = match var {
                Some(sym) => crate::execution::builtins::patterns::substitute_symbol(session, body, sym, value),
                None => body,
            };
            match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(instantiated)) {
                Ok(module) => {
                    let result_id = self.execute(session, &module, None)?;
                    let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(instantiated);
                    out.push(term);
                }
                Err(_) => out.push(instantiated),
            }
        }
        Ok(Some(out))
    }

    pub(crate) fn slot_as_term(&self, session: &mut Session, slot: Slot) -> Result<TermId> {
        match slot {
            Slot::Term(term) => Ok(term),
            Slot::Boolean(value) => Ok(session.builder().boolean(value, Default::default())),
            Slot::Symbol(symbol) => Ok(session.builder().symbol_id(symbol, Default::default())),
            Slot::Unit => Ok(session.builder().null(Default::default())),
            Slot::Scope(_) | Slot::Result(_) | Slot::Value(_) | Slot::Empty => Err(diag("slot_not_term")),
        }
    }
}
