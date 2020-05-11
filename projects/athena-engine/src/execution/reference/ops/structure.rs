//! 列表 / 矩阵 / 算术算子求值。

use athena_ir::SemanticOperator;
use athena_vm::SlotTable;
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};

use super::super::{IndexStep, ReferenceExecutor, Slot, helpers::*};
use crate::{
    api::request::AthenaRequest,
    domains::linear_algebra::det_bareiss,
    execution::{compiler::ExecutionCompiler, ir::SsaValueId, number_of, push_semantic},
    runtime::{
        session::Session,
        values::{
            arena::push_list,
            numeric_clone::clone_rational,
        },
    },
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
        if args.len() != 1 {
            return Ok(Slot::Term({
                let mut terms = Vec::with_capacity(args.len());
                for id in args {
                    let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
                    terms.push(self.slot_as_term(session, slot)?);
                }
                push_semantic(session, SemanticOperator::Sum, terms)
            }));
        }
        let term = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let Some(athena_ir::TermNode::Collection { elements: items, .. }) = session.arena.get(term)
        else {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::Sum, vec![term])));
        };
        let items = items.clone();
        if items.is_empty() {
            return Ok(Slot::Term(session.builder().int(0, Default::default())));
        }
        if matches!(session.arena.get(items[0]), Some(athena_ir::TermNode::Collection { elements: _, .. })) {
            // 矩阵：各列求和成行向量。
            let Some((_, cols)) = nested_list_shape(session, term)
            else {
                return Ok(Slot::Term(push_semantic(session, SemanticOperator::Sum, vec![term])));
            };
            let mut out = Vec::with_capacity(cols as usize);
            for j in 0..cols as usize {
                let mut col = Vec::with_capacity(items.len());
                for row in &items {
                    let cell = match session.arena.get(*row) {
                        Some(athena_ir::TermNode::Collection { elements: cells, .. }) => cells.get(j).copied(),
                        _ => None,
                    };
                    let Some(cell) = cell
                    else {
                        return Ok(Slot::Term(push_semantic(session, SemanticOperator::Sum, vec![term])));
                    };
                    col.push(cell);
                }
                out.push(fold_plus_symbolic(session, col));
            }
            return Ok(Slot::Term(push_list(session, out)));
        }
        // 向量：经 Plus 折叠求标量和。
        Ok(Slot::Term(fold_plus_symbolic(session, items)))
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

    pub(crate) fn eval_det(
        &self,
        session: &mut Session,
        args: &[SsaValueId],
        slots: &SlotTable,
        invalid: &mut Option<Diagnostic>,
    ) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let term = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let echo = push_semantic(session, SemanticOperator::Determinant, vec![term]);
        let Some(matrix) = term_to_rational_matrix_session(session, term)
        else {
            return Ok(Slot::Term(echo));
        };
        match det_bareiss(&matrix) {
            Ok(result) => Ok(Slot::Term(rational_to_term_session(session, &result.det))),
            Err(diagnostic) => {
                *invalid = Some(diagnostic);
                Ok(Slot::Term(echo))
            }
        }
    }

    pub(crate) fn eval_range(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            terms.push(self.slot_as_term(session, slot)?);
        }
        let ints = terms.iter().map(|t| number_of(session, *t).and_then(|n| n.as_exact_integer())).collect::<Option<Vec<_>>>();
        let Some(ints) = ints
        else {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::Range, terms)));
        };
        let bounds = match ints.as_slice() {
            [n] => Some((1, *n, 1)),
            [a, b] => Some((*a, *b, 1)),
            [a, b, step] => Some((*a, *b, *step)),
            _ => None,
        };
        let Some((a, b, step)) = bounds
        else {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::Range, terms)));
        };
        let Some(values) = expand_span_3(a, step, b)
        else {
            return Ok(Slot::Term(push_semantic(session, SemanticOperator::Range, terms)));
        };
        let out: Vec<TermId> = values.into_iter().map(|v| session.builder().int(v, Default::default())).collect();
        Ok(Slot::Term(push_list(session, out)))
    }

    /// 对目标 SSA 值执行中立 [`IndexSpec`] 轴。
    pub(crate) fn eval_index(
        &self,
        session: &mut Session,
        target: SsaValueId,
        axes: &[athena_types::IndexSpec],
        slots: &SlotTable,
        invalid: &mut Option<Diagnostic>,
    ) -> Result<Slot> {
        use athena_types::IndexSpec;

        let slot = slots.get(target.0).ok_or_else(|| diag("index_target_undefined"))?;
        let mut cur = self.slot_as_term(session, slot)?;

        // 先 `All`，再对每行应用剩余轴（列 / 嵌套选择）。
        if let [IndexSpec::All, rest @ ..] = axes {
            if !rest.is_empty() {
                if let Some(athena_ir::TermNode::Collection { elements: rows, .. }) = session.arena.get(cur) {
                    let rows = rows.clone();
                    let mut out = Vec::with_capacity(rows.len());
                    for row in rows {
                        let mut cell = row;
                        for axis in rest {
                            match self.index_one(session, cell, axis)? {
                                IndexStep::Next(next) => cell = next,
                                IndexStep::Residual => return Ok(Slot::Term(cur)),
                                IndexStep::Invalid { echo, diagnostic } => {
                                    *invalid = Some(diagnostic);
                                    return Ok(Slot::Term(echo));
                                }
                            }
                        }
                        out.push(cell);
                    }
                    return Ok(Slot::Term(push_list(session, out)));
                }
            }
        }

        for axis in axes {
            match self.index_one(session, cur, axis)? {
                IndexStep::Next(next) => cur = next,
                IndexStep::Residual => return Ok(Slot::Term(cur)),
                IndexStep::Invalid { echo, diagnostic } => {
                    *invalid = Some(diagnostic);
                    return Ok(Slot::Term(echo));
                }
            }
        }
        Ok(Slot::Term(cur))
    }

    /// 应用一条 [`IndexSpec`] 轴（1-based 标量、`All`、`EndRelative`、`Range`）。
    pub(crate) fn index_one(&self, session: &mut Session, expr: TermId, spec: &athena_types::IndexSpec) -> Result<IndexStep> {
        use athena_types::{IndexSpec, IntegerIndex, IntegerOffset};

        let items = match session.arena.get(expr) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
            Some(athena_ir::TermNode::Application { arguments, .. }) => arguments.clone(),
            _ => return Ok(IndexStep::Residual),
        };
        let len = items.len();

        match spec {
            IndexSpec::All => Ok(IndexStep::Next(push_list(session, items))),
            IndexSpec::EndRelative(IntegerOffset(off)) => {
                let pos = len as i64 + *off - 1;
                if pos < 0 || pos as usize >= len {
                    return Ok(IndexStep::Invalid {
                        echo: expr,
                        diagnostic: crate::diagnostics::invalid_index_diagnostic(*off, Some(len as u64)),
                    });
                }
                Ok(IndexStep::Next(items[pos as usize]))
            }
            IndexSpec::Scalar(IntegerIndex(idx)) => {
                if *idx == 0 {
                    // 绝不经显示名符号具体化头。
                    // 下标 0 得到同头 / 同集合种类的类型化空投影。
                    return Ok(IndexStep::Next(match session.arena.get(expr) {
                        Some(athena_ir::TermNode::Collection { kind, .. }) => {
                            let kind = *kind;
                            let span = athena_ir::TermNode::default_span();
                            session.arena.push(athena_ir::TermNode::Collection { kind, elements: Vec::new() }, span)
                        }
                        Some(athena_ir::TermNode::Application { head, .. }) => {
                            let head = *head;
                            let span = athena_ir::TermNode::default_span();
                            session.arena.push(athena_ir::TermNode::Application { head, arguments: Vec::new() }, span)
                        }
                        _ => return Ok(IndexStep::Residual),
                    }));
                }
                let pos = if *idx > 0 {
                    (*idx - 1) as usize
                }
                else {
                    let pos = len as i64 + *idx;
                    if pos < 0 {
                        return Ok(IndexStep::Invalid {
                            echo: expr,
                            diagnostic: crate::diagnostics::invalid_index_diagnostic(*idx, Some(len as u64)),
                        });
                    }
                    pos as usize
                };
                match items.get(pos) {
                    Some(item) => Ok(IndexStep::Next(*item)),
                    None => {
                        Ok(IndexStep::Invalid { echo: expr, diagnostic: crate::diagnostics::invalid_index_diagnostic(*idx, Some(len as u64)) })
                    }
                }
            }
            IndexSpec::Range { start, end, step } => {
                let Some(values) = expand_span_3(start.0, *step, end.0)
                else {
                    return Ok(IndexStep::Residual);
                };
                let mut out = Vec::with_capacity(values.len());
                for v in values {
                    match self.index_one(session, expr, &IndexSpec::Scalar(IntegerIndex(v)))? {
                        IndexStep::Next(item) => out.push(item),
                        IndexStep::Residual => return Ok(IndexStep::Residual),
                        IndexStep::Invalid { echo, diagnostic } => {
                            return Ok(IndexStep::Invalid { echo, diagnostic });
                        }
                    }
                }
                Ok(IndexStep::Next(push_list(session, out)))
            }
            IndexSpec::Cartesian(axes) => {
                let mut cur = expr;
                for axis in axes {
                    match self.index_one(session, cur, axis)? {
                        IndexStep::Next(next) => cur = next,
                        other => return Ok(other),
                    }
                }
                Ok(IndexStep::Next(cur))
            }
            IndexSpec::DomainSpecific(_) => Ok(IndexStep::Residual),
        }
    }

    pub(crate) fn eval_join(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        let mut out = Vec::new();
        let mut terms = Vec::with_capacity(args.len());
        for id in args {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            let term = self.slot_as_term(session, slot)?;
            terms.push(term);
            match session.arena.get(term) {
                Some(athena_ir::TermNode::Collection { elements: items, .. }) => out.extend_from_slice(items),
                _ => return Ok(Slot::Term(push_semantic(session, SemanticOperator::Join, terms))),
            }
        }
        Ok(Slot::Term(push_list(session, out)))
    }

    pub(crate) fn eval_compare_chain(
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
        Ok(match evaluate_compare_terms(session, op, terms)? {
            CompareOutcome::Boolean(v) => Slot::Boolean(v),
            CompareOutcome::Term(term) => Slot::Term(term),
        })
    }

    /// 带标量广播的逐元 `DotTimes` / `DotDivide` / `DotPower`。
    pub(crate) fn eval_dot_arithmetic(
        &self,
        session: &mut Session,
        op: SemanticOperator,
        args: &[SsaValueId],
        slots: &SlotTable,
    ) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let left = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let right = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let echo = push_semantic(session, op, vec![left, right]);
        let scalar_op = match op {
            SemanticOperator::ElementwiseMultiply => SemanticOperator::Multiply,
            SemanticOperator::ElementwiseDivide => SemanticOperator::Divide,
            SemanticOperator::ElementwisePower => SemanticOperator::Power,
            _ => return Ok(Slot::Term(echo)),
        };
        match self.dot_zip_eval(session, scalar_op, left, right)? {
            Some(term) => Ok(Slot::Term(term)),
            None => Ok(Slot::Term(echo)),
        }
    }

    /// 递归 zip 集合（带标量广播），再成对求值 `scalar_op`。
    fn dot_zip_eval(&self, session: &mut Session, scalar_op: SemanticOperator, left: TermId, right: TermId) -> Result<Option<TermId>> {
        let left_is_collection = matches!(session.arena.get(left), Some(athena_ir::TermNode::Collection { .. }));
        let right_is_collection = matches!(session.arena.get(right), Some(athena_ir::TermNode::Collection { .. }));
        match (left_is_collection, right_is_collection) {
            (true, true) => {
                let a = match session.arena.get(left) {
                    Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                    _ => return Ok(None),
                };
                let b = match session.arena.get(right) {
                    Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                    _ => return Ok(None),
                };
                if a.len() != b.len() {
                    return Ok(None);
                }
                let mut out = Vec::with_capacity(a.len());
                for (lhs, rhs) in a.into_iter().zip(b.into_iter()) {
                    match self.dot_zip_eval(session, scalar_op, lhs, rhs)? {
                        Some(term) => out.push(term),
                        None => return Ok(None),
                    }
                }
                Ok(Some(push_list(session, out)))
            }
            (true, false) => {
                let a = match session.arena.get(left) {
                    Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                    _ => return Ok(None),
                };
                let mut out = Vec::with_capacity(a.len());
                for lhs in a {
                    match self.dot_zip_eval(session, scalar_op, lhs, right)? {
                        Some(term) => out.push(term),
                        None => return Ok(None),
                    }
                }
                Ok(Some(push_list(session, out)))
            }
            (false, true) => {
                let b = match session.arena.get(right) {
                    Some(athena_ir::TermNode::Collection { elements, .. }) => elements.clone(),
                    _ => return Ok(None),
                };
                let mut out = Vec::with_capacity(b.len());
                for rhs in b {
                    match self.dot_zip_eval(session, scalar_op, left, rhs)? {
                        Some(term) => out.push(term),
                        None => return Ok(None),
                    }
                }
                Ok(Some(push_list(session, out)))
            }
            (false, false) => {
                let app = push_semantic(session, scalar_op, vec![left, right]);
                match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(app)) {
                    Ok(module) => {
                        let result_id = self.execute(session, &module, None)?;
                        Ok(Some(session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(app)))
                    }
                    Err(_) => Ok(Some(app)),
                }
            }
        }
    }

    pub(crate) fn eval_arithmetic(
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
        Ok(Slot::Term(evaluate_arithmetic_terms(session, op, terms)?))
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
