//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期覆盖 Boolean、标量算术 / 比较 / 一元、`Join` / `Range`、session / 局部 binding、
//! scope 帧栈、`Index`，以及经 [`ProviderCallDescriptor::payload`] 绑定的 `CallProvider`。

use athena_ir::{ApplicationHead, SemanticOperator, TermNode};
use athena_numeric::compare as num_compare;
use athena_types::{BindingEvaluationPolicy, BindingKind, CollectionKind, Diagnostic, DiagnosticCode, IndexSpec, Result, SymbolId, TermId};
use athena_vm::{ExtensionOpId, HostOutcome, IndexAxesId, ProviderOpId, SemanticOpId, SlotValue, VmHost};

use crate::{
    api::request::AthenaRequest,
    reasoning::mgraph::execute_domain_via_semantic_entry,
    execution::{
        LocalBinding, ScopeFrame, execute_ir_request,
        ir::ProviderCallDescriptor,
        number_of,
        provider::ProviderCallHandoff,
        push_semantic,
        reference::{
            CompareOutcome, IndexOutcome, MatrixStoreOutcome, compare_list_broadcast, domain_result_symbolic_term, evaluate_apply_head_terms,
            evaluate_apply_terms, evaluate_arithmetic_terms, evaluate_collect_matches_terms, evaluate_compare_terms,
            evaluate_determinant_term, evaluate_elementwise_terms, evaluate_extension_apply_terms, evaluate_index_axes,
            evaluate_index_axes_matrix, evaluate_join_terms, evaluate_map_indexed_terms, evaluate_map_terms, evaluate_map_thread_terms, evaluate_matches_terms,
            evaluate_take_terms, evaluate_drop_terms, evaluate_append_terms, evaluate_prepend_terms,
            evaluate_member_q_terms, evaluate_sort_terms, evaluate_delete_duplicates_terms,
            evaluate_count_terms, evaluate_partition_terms, evaluate_constant_array_terms, evaluate_union_terms,
            evaluate_intersection_terms, evaluate_accumulate_terms, evaluate_differences_terms, evaluate_free_q_terms,
            evaluate_extract_terms, evaluate_pad_left_terms, evaluate_riffle_terms, evaluate_position_terms, evaluate_array_terms,
            evaluate_matrix_constructor_terms,
            evaluate_diagonal_matrix_terms, evaluate_product_iterator_terms, evaluate_product_terms, evaluate_range_terms, evaluate_replace_all_terms,
            evaluate_rule_terms, evaluate_simplify_terms, evaluate_size_terms, evaluate_special_unary_terms,
            evaluate_sum_iterator_terms, evaluate_sum_terms, evaluate_unary_term, slot_as_boolean_like, store_index_axes,
            store_index_axes_matrix, symbolic_term_from_value_id,
            domain_request_residual_term, linear_algebra_missing_binding,
        },
    },
    runtime::{results::computation_from_domain, session::Session, values::numeric_clone::clone_number},
};

/// 执行宿主（engine 在 VM 之上 · 不拥有解释循环）。
#[derive(Debug)]
pub struct ExecutionHost<'a> {
    session: &'a mut Session,
    frames: FrameStorage<'a>,
    provider_calls: Vec<ProviderCallDescriptor>,
    index_axes: Vec<Vec<IndexSpec>>,
}

#[derive(Debug)]
enum FrameStorage<'a> {
    Owned(Vec<ScopeFrame>),
    Borrowed(&'a mut Vec<ScopeFrame>),
}

impl FrameStorage<'_> {
    fn as_slice(&self) -> &[ScopeFrame] {
        match self {
            Self::Owned(frames) => frames.as_slice(),
            Self::Borrowed(frames) => frames.as_slice(),
        }
    }

    fn as_mut_vec(&mut self) -> &mut Vec<ScopeFrame> {
        match self {
            Self::Owned(frames) => frames,
            Self::Borrowed(frames) => frames,
        }
    }
}

impl<'a> ExecutionHost<'a> {
    /// 构造（持有 session；领域载荷经 [`ProviderCallDescriptor::payload`] 解析）。
    pub fn new(session: &'a mut Session, provider_calls: Vec<ProviderCallDescriptor>, index_axes: Vec<Vec<IndexSpec>>) -> Self {
        Self { session, frames: FrameStorage::Owned(Vec::new()), provider_calls, index_axes }
    }

    /// 与 Reference / 外部作用域帧栈共享同一 `frames`（收窄第二套循环）。
    pub fn with_shared_frames(
        session: &'a mut Session,
        frames: &'a mut Vec<ScopeFrame>,
        provider_calls: Vec<ProviderCallDescriptor>,
        index_axes: Vec<Vec<IndexSpec>>,
    ) -> Self {
        Self { session, frames: FrameStorage::Borrowed(frames), provider_calls, index_axes }
    }

    fn unsupported(op: SemanticOpId) -> HostOutcome {
        HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "apply_semantic_unsupported")
                .detail("op", op.0),
        )
    }

    fn unknown_op(op: SemanticOpId) -> HostOutcome {
        HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "apply_semantic_unknown_op")
                .detail("op", op.0),
        )
    }

    /// 未知 / 未展开语义 → 残差应用（不回退 Reference）。
    fn apply_residual_echo(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        if op.as_unary().is_some() {
            return self.outcome_special_unary(op, terms);
        }
        let term = push_semantic(self.session, op, terms);
        Ok(HostOutcome::Residual(SlotValue::Term(term)))
    }

    fn slot_as_term(&mut self, slot: SlotValue) -> Result<TermId> {
        match slot {
            SlotValue::Term(term) => {
                let term_ref = self.session.arena.term_ref(term).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionHost")
                        .detail("reason", "term_out_of_range")
                })?;
                self.session.arena.check_ref(term_ref)
            }
            SlotValue::Boolean(value) => Ok(self.session.builder().boolean(value, Default::default())),
            SlotValue::Symbol(symbol) => Ok(self.session.builder().symbol_id(symbol, Default::default())),
            SlotValue::Unit => Ok(self.session.builder().null(Default::default())),
            SlotValue::Value(value_id) => symbolic_term_from_value_id(self.session, value_id),
            other => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "slot_not_term_like")
                .detail("slot", format!("{other:?}"))),
        }
    }

    fn apply_arithmetic(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_arithmetic_terms(self.session, op, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_compare(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        Ok(match evaluate_compare_terms(self.session, op, terms)? {
            CompareOutcome::Boolean(v) => HostOutcome::Value(SlotValue::Boolean(v)),
            CompareOutcome::Term(term) => HostOutcome::Value(SlotValue::Term(term)),
        })
    }

    fn apply_unary(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(op.discriminant())));
        }
        let term = self.slot_as_term(args[0])?;
        let out = evaluate_unary_term(self.session, op, term)?;
        Ok(HostOutcome::Value(SlotValue::Term(out)))
    }

    fn bind_term(&mut self, symbol: SymbolId, term: TermId, residual: bool) {
        if let Some(frame) = self.frames.as_mut_vec().last_mut() {
            // 局部帧只存已物化值；残差策略在局部作用域内仍立即绑定 Value。
            let _ = residual;
            frame.bind(symbol, LocalBinding::Value(term));
            return;
        }
        if residual {
            self.session.defs.write_residual_binding(symbol, term);
        }
        else {
            self.session.defs.write_binding(symbol, term);
        }
    }

    fn bind_matrix(&mut self, symbol: SymbolId, matrix: crate::domains::linear_algebra::MatrixRef) {
        if let Some(frame) = self.frames.as_mut_vec().last_mut() {
            frame.bind(symbol, LocalBinding::Matrix(matrix));
            return;
        }
        self.session.bind_matrix(symbol, matrix);
    }

    fn apply_join(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_join_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_take(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Take.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let count = self.slot_as_term(args[1])?;
        let term = evaluate_take_terms(self.session, list, count)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_drop(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Drop.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let count = self.slot_as_term(args[1])?;
        let term = evaluate_drop_terms(self.session, list, count)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_append(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Append.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let elem = self.slot_as_term(args[1])?;
        let term = evaluate_append_terms(self.session, list, elem)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_prepend(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Prepend.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let elem = self.slot_as_term(args[1])?;
        let term = evaluate_prepend_terms(self.session, list, elem)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_member_q(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::MemberQ.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let elem = self.slot_as_term(args[1])?;
        let term = evaluate_member_q_terms(self.session, list, elem)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_sort(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Sort.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let term = evaluate_sort_terms(self.session, list)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_delete_duplicates(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::DeleteDuplicates.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let term = evaluate_delete_duplicates_terms(self.session, list)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_count(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Count.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let elem = self.slot_as_term(args[1])?;
        let term = evaluate_count_terms(self.session, list, elem)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_partition(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Partition.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let size = self.slot_as_term(args[1])?;
        let term = evaluate_partition_terms(self.session, list, size)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_constant_array(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::ConstantArray.discriminant())));
        }
        let elem = self.slot_as_term(args[0])?;
        let count = self.slot_as_term(args[1])?;
        let term = evaluate_constant_array_terms(self.session, elem, count)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_union(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_union_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_intersection(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_intersection_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_accumulate(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Accumulate.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let term = evaluate_accumulate_terms(self.session, list)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_differences(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Differences.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let term = evaluate_differences_terms(self.session, list)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_free_q(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::FreeQ.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let elem = self.slot_as_term(args[1])?;
        let term = evaluate_free_q_terms(self.session, list, elem)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_extract(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Extract.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let index = self.slot_as_term(args[1])?;
        let term = evaluate_extract_terms(self.session, list, index)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_pad_left(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::PadLeft.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let len = self.slot_as_term(args[1])?;
        let term = evaluate_pad_left_terms(self.session, list, len)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_riffle(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Riffle.discriminant())));
        }
        let left = self.slot_as_term(args[0])?;
        let right = self.slot_as_term(args[1])?;
        let term = evaluate_riffle_terms(self.session, left, right)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_position(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Position.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let elem = self.slot_as_term(args[1])?;
        let term = evaluate_position_terms(self.session, list, elem)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_array(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Array.discriminant())));
        }
        let func = self.slot_as_term(args[0])?;
        let count = self.slot_as_term(args[1])?;
        let term = evaluate_array_terms(self.session, func, count)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_range(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_range_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_size(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_size_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_sum(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() == 2 {
            let body = self.slot_as_term(args[0])?;
            let iter = self.slot_as_term(args[1])?;
            let term = evaluate_sum_iterator_terms(self.session, body, iter)?;
            return Ok(HostOutcome::Value(SlotValue::Term(term)));
        }
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_sum_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_product(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() == 2 {
            let body = self.slot_as_term(args[0])?;
            let iter = self.slot_as_term(args[1])?;
            let term = evaluate_product_iterator_terms(self.session, body, iter)?;
            return Ok(HostOutcome::Value(SlotValue::Term(term)));
        }
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_product_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_determinant(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Determinant.discriminant())));
        }
        let term = self.slot_as_term(args[0])?;
        let (out, diag_opt) = evaluate_determinant_term(self.session, term)?;
        // Bareiss 失败：SoftInvalid（VM 解释器提升为硬 Diagnostic，与 Index OOB 同合同）。
        if let Some(diagnostic) = diag_opt {
            return Ok(HostOutcome::SoftInvalid { value: SlotValue::Term(out), diagnostic });
        }
        Ok(HostOutcome::Value(SlotValue::Term(out)))
    }

    fn apply_matrix_constructor(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_matrix_constructor_terms(self.session, op, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_diagonal_matrix(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_diagonal_matrix_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_elementwise(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(op.discriminant())));
        }
        let left = self.slot_as_term(args[0])?;
        let right = self.slot_as_term(args[1])?;
        let term = evaluate_elementwise_terms(self.session, op, left, right)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    /// `Not` / `TrueQ` / `And` / `Or`：Boolean 原子与精确 `0`/`1` truthiness；否则残差。
    fn apply_logical(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut bools = Vec::with_capacity(args.len());
        for slot in args {
            match slot_as_boolean_like(self.session, *slot) {
                Some(v) => bools.push(v),
                None => {
                    let mut terms = Vec::with_capacity(args.len());
                    for slot in args {
                        terms.push(self.slot_as_term(*slot)?);
                    }
                    let echo = push_semantic(self.session, op, terms);
                    return Ok(HostOutcome::Residual(SlotValue::Term(echo)));
                }
            }
        }
        let result = match (op, bools.as_slice()) {
            (SemanticOperator::Not, [a]) => !*a,
            (SemanticOperator::TrueQ, [a]) => *a,
            (SemanticOperator::And, values) => values.iter().copied().all(|v| v),
            (SemanticOperator::Or, values) => values.iter().copied().any(|v| v),
            _ => {
                return Ok(Self::unsupported(SemanticOpId(op.discriminant())));
            }
        };
        Ok(HostOutcome::Value(SlotValue::Boolean(result)))
    }

    /// `Map[func, list]` — 经共享 helper，元素再求值走 `execute_ir_request`。
    fn apply_map(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Map.discriminant())));
        }
        let func = self.slot_as_term(args[0])?;
        let list = self.slot_as_term(args[1])?;
        let term = evaluate_map_terms(self.session, func, list)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    /// `MapIndexed[func, list]` — `func[elem, {i}]`。
    fn apply_map_indexed(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::MapIndexed.discriminant())));
        }
        let func = self.slot_as_term(args[0])?;
        let list = self.slot_as_term(args[1])?;
        let term = evaluate_map_indexed_terms(self.session, func, list)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    /// `MapThread[func, {list₁,…}]` — 按列 zip 应用。
    fn apply_map_thread(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::MapThread.discriminant())));
        }
        let func = self.slot_as_term(args[0])?;
        let lists = self.slot_as_term(args[1])?;
        let term = evaluate_map_thread_terms(self.session, func, lists)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    /// `Apply[head, list]`。
    fn apply_apply(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Apply.discriminant())));
        }
        let head = self.slot_as_term(args[0])?;
        let second = self.slot_as_term(args[1])?;
        let term = evaluate_apply_terms(self.session, head, second)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    /// `ApplyHead[head, args…]`。
    fn apply_apply_head(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.is_empty() {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::ApplyHead.discriminant())));
        }
        let head = self.slot_as_term(args[0])?;
        let mut call_args = Vec::with_capacity(args.len().saturating_sub(1));
        for slot in &args[1..] {
            call_args.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_apply_head_terms(self.session, head, call_args)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    /// `Function[…]` — 构造残差（不求值），供 `Map` 等引用。
    fn apply_function_form(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = push_semantic(self.session, SemanticOperator::Function, terms);
        Ok(HostOutcome::Residual(SlotValue::Term(term)))
    }

    fn apply_rule(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(op.discriminant())));
        }
        let lhs = self.slot_as_term(args[0])?;
        let rhs = self.slot_as_term(args[1])?;
        let term = evaluate_rule_terms(self.session, op, lhs, rhs)?;
        Ok(HostOutcome::Residual(SlotValue::Term(term)))
    }

    fn apply_replace_all(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::ReplaceAll.discriminant())));
        }
        let expr = self.slot_as_term(args[0])?;
        let rules = self.slot_as_term(args[1])?;
        let term = evaluate_replace_all_terms(self.session, expr, rules)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_matches(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Matches.discriminant())));
        }
        let expr = self.slot_as_term(args[0])?;
        let pat = self.slot_as_term(args[1])?;
        let matched = evaluate_matches_terms(self.session, expr, pat)?;
        Ok(HostOutcome::Value(SlotValue::Boolean(matched)))
    }

    fn apply_collect_matches(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::CollectMatches.discriminant())));
        }
        let list = self.slot_as_term(args[0])?;
        let pat = self.slot_as_term(args[1])?;
        let term = evaluate_collect_matches_terms(self.session, list, pat)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_simplify(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Simplify.discriminant())));
        }
        let expr = self.slot_as_term(args[0])?;
        let term = evaluate_simplify_terms(self.session, expr)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_special_unary(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        self.outcome_special_unary(op, terms)
    }

    /// 精确 / machine 折叠 → Value；仍为一元应用残差 → Residual（不得抬 Exact）。
    fn outcome_special_unary(&mut self, op: SemanticOperator, terms: Vec<TermId>) -> Result<HostOutcome> {
        let term = evaluate_special_unary_terms(self.session, op, terms)?;
        let residual = matches!(
            self.session.arena.get(term),
            Some(TermNode::Application { head: ApplicationHead::Semantic(sem), .. }) if sem.as_unary().is_some()
        );
        if residual {
            Ok(HostOutcome::Residual(SlotValue::Term(term)))
        } else {
            Ok(HostOutcome::Value(SlotValue::Term(term)))
        }
    }

    /// `Identical` 结构比较。`Equal` / `Unequal`：可判定原子 → Boolean，否则残差项（不静默 `False`）。
    fn apply_equality(&mut self, op: SemanticOperator, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 2 {
            return Ok(Self::unsupported(SemanticOpId(op.discriminant())));
        }
        let left = args[0];
        let right = args[1];
        if op == SemanticOperator::Identical {
            let same = match (left, right) {
                (SlotValue::Boolean(a), SlotValue::Boolean(b)) => a == b,
                (SlotValue::Symbol(a), SlotValue::Symbol(b)) => a == b,
                (SlotValue::Term(a), SlotValue::Term(b)) => {
                    let a = self.slot_as_term(SlotValue::Term(a))?;
                    let b = self.slot_as_term(SlotValue::Term(b))?;
                    self.session.arena.structural_eq(a, b)
                }
                (SlotValue::Unit, SlotValue::Unit) => true,
                _ => false,
            };
            return Ok(HostOutcome::Value(SlotValue::Boolean(same)));
        }
        let bool_out = |eq: bool| -> HostOutcome {
            let v = if op == SemanticOperator::Unequal { !eq } else { eq };
            HostOutcome::Value(SlotValue::Boolean(v))
        };
        match (left, right) {
            (SlotValue::Boolean(a), SlotValue::Boolean(b)) => Ok(bool_out(a == b)),
            (SlotValue::Symbol(a), SlotValue::Symbol(b)) => Ok(bool_out(a == b)),
            (SlotValue::Unit, SlotValue::Unit) => Ok(bool_out(true)),
            (SlotValue::Term(a), SlotValue::Term(b)) => {
                let a = self.slot_as_term(SlotValue::Term(a))?;
                let b = self.slot_as_term(SlotValue::Term(b))?;
                if self.session.arena.structural_eq(a, b) {
                    return Ok(bool_out(true));
                }
                let pick = match op {
                    SemanticOperator::Equal => |o: core::cmp::Ordering| o == core::cmp::Ordering::Equal,
                    SemanticOperator::Unequal => |o: core::cmp::Ordering| o != core::cmp::Ordering::Equal,
                    _ => {
                        let echo = push_semantic(self.session, op, vec![a, b]);
                        return Ok(HostOutcome::Residual(SlotValue::Term(echo)));
                    }
                };
                if let Some(broadcast) = compare_list_broadcast(self.session, op, a, b, pick)? {
                    return Ok(HostOutcome::Value(SlotValue::Term(broadcast)));
                }
                let na = number_of(self.session, a).map(clone_number);
                let nb = number_of(self.session, b).map(clone_number);
                if let (Some(left_n), Some(right_n)) = (na, nb) {
                    let ord = num_compare(&left_n, &right_n).ok_or_else(|| {
                        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ExecutionHost")
                            .detail("reason", "compare_failed")
                    })?;
                    return Ok(bool_out(ord == core::cmp::Ordering::Equal));
                }
                let echo = push_semantic(self.session, op, vec![a, b]);
                Ok(HostOutcome::Residual(SlotValue::Term(echo)))
            }
            _ => {
                let a = self.slot_as_term(left)?;
                let b = self.slot_as_term(right)?;
                let echo = push_semantic(self.session, op, vec![a, b]);
                Ok(HostOutcome::Residual(SlotValue::Term(echo)))
            }
        }
    }

    /// `RegisterRuleDispatch`：结构 pattern 编译后挂到扩展头。
    pub fn register_rule_dispatch(
        &mut self,
        head: SlotValue,
        operator: athena_types::ExtensionOperatorId,
        pattern: SlotValue,
        replacement: SlotValue,
    ) -> Result<HostOutcome> {
        let SlotValue::Symbol(symbol) = head
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "write_key_not_symbol"),
            ));
        };
        let SlotValue::Term(pattern_term) = pattern
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "write_pattern_not_term"),
            ));
        };
        let SlotValue::Term(value_term) = replacement
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "write_value_unsupported"),
            ));
        };
        let compiled = crate::execution::builtins::patterns::structural_pattern_from_term(self.session, pattern_term);
        self.session.defs.register_extension_rule_for_symbol(symbol, operator, compiled, value_term);
        Ok(HostOutcome::Value(SlotValue::Unit))
    }

    /// `RegisterCompiledRule`：把 Session 已编译规则挂到分派表。
    pub fn register_compiled_rule(&mut self, table: athena_types::DispatchTableId, rule: athena_types::CompiledRuleId) -> Result<HostOutcome> {
        let Some((pattern, replacement)) =
            self.session.compiled_rules.get(rule).map(|(pattern, replacement)| (pattern.owning_copy(), *replacement))
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "compiled_rule_missing"),
            ));
        };
        self.session.defs.append_rule(table, pattern, replacement);
        Ok(HostOutcome::Value(SlotValue::Unit))
    }

    /// 扩展算子：down-value 命中 → Value；否则 Residual。
    pub fn apply_extension_operator(&mut self, op: athena_types::ExtensionOperatorId, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let (term, residual) = evaluate_extension_apply_terms(self.session, op, terms)?;
        if residual { Ok(HostOutcome::Residual(SlotValue::Term(term))) } else { Ok(HostOutcome::Value(SlotValue::Term(term))) }
    }
}

impl VmHost for ExecutionHost<'_> {
    fn apply_semantic(&mut self, op: SemanticOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        if op.0 == SemanticOperator::Not.discriminant() {
            return self.apply_logical(SemanticOperator::Not, args);
        }
        if op.0 == SemanticOperator::TrueQ.discriminant() {
            return self.apply_logical(SemanticOperator::TrueQ, args);
        }
        if op.0 == SemanticOperator::And.discriminant() {
            return self.apply_logical(SemanticOperator::And, args);
        }
        if op.0 == SemanticOperator::Or.discriminant() {
            return self.apply_logical(SemanticOperator::Or, args);
        }
        if op.0 == SemanticOperator::Equal.discriminant()
            || op.0 == SemanticOperator::Unequal.discriminant()
            || op.0 == SemanticOperator::Identical.discriminant()
        {
            let op = if op.0 == SemanticOperator::Equal.discriminant() {
                SemanticOperator::Equal
            }
            else if op.0 == SemanticOperator::Unequal.discriminant() {
                SemanticOperator::Unequal
            }
            else {
                SemanticOperator::Identical
            };
            return self.apply_equality(op, args);
        }
        if op.0 == SemanticOperator::Add.discriminant() {
            return self.apply_arithmetic(SemanticOperator::Add, args);
        }
        if op.0 == SemanticOperator::Multiply.discriminant() {
            return self.apply_arithmetic(SemanticOperator::Multiply, args);
        }
        if op.0 == SemanticOperator::Subtract.discriminant() {
            return self.apply_arithmetic(SemanticOperator::Subtract, args);
        }
        if op.0 == SemanticOperator::Negate.discriminant() {
            return self.apply_arithmetic(SemanticOperator::Negate, args);
        }
        if op.0 == SemanticOperator::Divide.discriminant() {
            return self.apply_arithmetic(SemanticOperator::Divide, args);
        }
        if op.0 == SemanticOperator::Power.discriminant() {
            return self.apply_arithmetic(SemanticOperator::Power, args);
        }
        if op.0 == SemanticOperator::Less.discriminant() {
            return self.apply_compare(SemanticOperator::Less, args);
        }
        if op.0 == SemanticOperator::Greater.discriminant() {
            return self.apply_compare(SemanticOperator::Greater, args);
        }
        if op.0 == SemanticOperator::LessEqual.discriminant() {
            return self.apply_compare(SemanticOperator::LessEqual, args);
        }
        if op.0 == SemanticOperator::GreaterEqual.discriminant() {
            return self.apply_compare(SemanticOperator::GreaterEqual, args);
        }
        if op.0 == SemanticOperator::Abs.discriminant() {
            return self.apply_unary(SemanticOperator::Abs, args);
        }
        if op.0 == SemanticOperator::Factorial.discriminant() {
            return self.apply_unary(SemanticOperator::Factorial, args);
        }
        if op.0 == SemanticOperator::Sqrt.discriminant() {
            return self.apply_unary(SemanticOperator::Sqrt, args);
        }
        if op.0 == SemanticOperator::Length.discriminant() {
            return self.apply_unary(SemanticOperator::Length, args);
        }
        if op.0 == SemanticOperator::First.discriminant() {
            return self.apply_unary(SemanticOperator::First, args);
        }
        if op.0 == SemanticOperator::Rest.discriminant() {
            return self.apply_unary(SemanticOperator::Rest, args);
        }
        if op.0 == SemanticOperator::Most.discriminant() {
            return self.apply_unary(SemanticOperator::Most, args);
        }
        if op.0 == SemanticOperator::Reverse.discriminant() {
            return self.apply_unary(SemanticOperator::Reverse, args);
        }
        if op.0 == SemanticOperator::Flatten.discriminant() {
            return self.apply_unary(SemanticOperator::Flatten, args);
        }
        if op.0 == SemanticOperator::Head.discriminant() {
            return self.apply_unary(SemanticOperator::Head, args);
        }
        if op.0 == SemanticOperator::Join.discriminant() {
            return self.apply_join(args);
        }
        if op.0 == SemanticOperator::Take.discriminant() {
            return self.apply_take(args);
        }
        if op.0 == SemanticOperator::Drop.discriminant() {
            return self.apply_drop(args);
        }
        if op.0 == SemanticOperator::Append.discriminant() {
            return self.apply_append(args);
        }
        if op.0 == SemanticOperator::Prepend.discriminant() {
            return self.apply_prepend(args);
        }
        if op.0 == SemanticOperator::MemberQ.discriminant() {
            return self.apply_member_q(args);
        }
        if op.0 == SemanticOperator::Sort.discriminant() {
            return self.apply_sort(args);
        }
        if op.0 == SemanticOperator::DeleteDuplicates.discriminant() {
            return self.apply_delete_duplicates(args);
        }
        if op.0 == SemanticOperator::Count.discriminant() {
            return self.apply_count(args);
        }
        if op.0 == SemanticOperator::Partition.discriminant() {
            return self.apply_partition(args);
        }
        if op.0 == SemanticOperator::ConstantArray.discriminant() {
            return self.apply_constant_array(args);
        }
        if op.0 == SemanticOperator::Union.discriminant() {
            return self.apply_union(args);
        }
        if op.0 == SemanticOperator::Intersection.discriminant() {
            return self.apply_intersection(args);
        }
        if op.0 == SemanticOperator::Accumulate.discriminant() {
            return self.apply_accumulate(args);
        }
        if op.0 == SemanticOperator::Differences.discriminant() {
            return self.apply_differences(args);
        }
        if op.0 == SemanticOperator::FreeQ.discriminant() {
            return self.apply_free_q(args);
        }
        if op.0 == SemanticOperator::Extract.discriminant() {
            return self.apply_extract(args);
        }
        if op.0 == SemanticOperator::PadLeft.discriminant() {
            return self.apply_pad_left(args);
        }
        if op.0 == SemanticOperator::Riffle.discriminant() {
            return self.apply_riffle(args);
        }
        if op.0 == SemanticOperator::Position.discriminant() {
            return self.apply_position(args);
        }
        if op.0 == SemanticOperator::Array.discriminant() {
            return self.apply_array(args);
        }
        if op.0 == SemanticOperator::Range.discriminant() {
            return self.apply_range(args);
        }
        if op.0 == SemanticOperator::Size.discriminant() {
            return self.apply_size(args);
        }
        if op.0 == SemanticOperator::Sum.discriminant() {
            return self.apply_sum(args);
        }
        if op.0 == SemanticOperator::Product.discriminant() {
            return self.apply_product(args);
        }
        if op.0 == SemanticOperator::Determinant.discriminant() {
            return self.apply_determinant(args);
        }
        if op.0 == SemanticOperator::Zeros.discriminant() {
            return self.apply_matrix_constructor(SemanticOperator::Zeros, args);
        }
        if op.0 == SemanticOperator::Ones.discriminant() {
            return self.apply_matrix_constructor(SemanticOperator::Ones, args);
        }
        if op.0 == SemanticOperator::Eye.discriminant() {
            return self.apply_matrix_constructor(SemanticOperator::Eye, args);
        }
        if op.0 == SemanticOperator::DiagonalMatrix.discriminant() {
            return self.apply_diagonal_matrix(args);
        }
        if op.0 == SemanticOperator::ElementwiseMultiply.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwiseMultiply, args);
        }
        if op.0 == SemanticOperator::ElementwiseDivide.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwiseDivide, args);
        }
        if op.0 == SemanticOperator::ElementwisePower.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwisePower, args);
        }
        if op.0 == SemanticOperator::ElementwiseAnd.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwiseAnd, args);
        }
        if op.0 == SemanticOperator::ElementwiseOr.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwiseOr, args);
        }
        if op.0 == SemanticOperator::Map.discriminant() {
            return self.apply_map(args);
        }
        if op.0 == SemanticOperator::MapIndexed.discriminant() {
            return self.apply_map_indexed(args);
        }
        if op.0 == SemanticOperator::MapThread.discriminant() {
            return self.apply_map_thread(args);
        }
        if op.0 == SemanticOperator::Apply.discriminant() {
            return self.apply_apply(args);
        }
        if op.0 == SemanticOperator::ApplyHead.discriminant() {
            return self.apply_apply_head(args);
        }
        if op.0 == SemanticOperator::Function.discriminant() {
            return self.apply_function_form(args);
        }
        if op.0 == SemanticOperator::Rule.discriminant() {
            return self.apply_rule(SemanticOperator::Rule, args);
        }
        if op.0 == SemanticOperator::RuleDeferred.discriminant() {
            return self.apply_rule(SemanticOperator::RuleDeferred, args);
        }
        if op.0 == SemanticOperator::ReplaceAll.discriminant() {
            return self.apply_replace_all(args);
        }
        if op.0 == SemanticOperator::Matches.discriminant() {
            return self.apply_matches(args);
        }
        if op.0 == SemanticOperator::CollectMatches.discriminant() {
            return self.apply_collect_matches(args);
        }
        if op.0 == SemanticOperator::Simplify.discriminant() {
            return self.apply_simplify(args);
        }
        if (100..=116).contains(&op.0) {
            if let Some(uf) = athena_ir::UnaryFunction::from_discriminant(op.0 - 100) {
                return self.apply_special_unary(SemanticOperator::Unary(uf), args);
            }
        }
        if let Some(sem) = SemanticOperator::from_discriminant(op.0) {
            return self.apply_residual_echo(sem, args);
        }
        Ok(Self::unknown_op(op))
    }

    fn call_provider(&mut self, op: ProviderOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = args;
        let descriptor = self.provider_calls.get(op.0 as usize).cloned().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "missing_provider_call")
                .detail("op", op.0)
        })?;
        if descriptor.id.0 != op.0 {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "provider_call_id_mismatch"),
            ));
        }
        let Some(payload_id) = descriptor.payload
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "provider_payload_unbound")
                    .detail("op", op.0),
            ));
        };
        let Some(domain) = self.session.domain_payloads.get(payload_id).map(|r| r.owning_copy())
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "provider_payload_missing")
                    .detail("op", op.0)
                    .detail("payload", payload_id.0),
            ));
        };
        let handoff = ProviderCallHandoff::from_descriptor(descriptor);
        // Goal→VM→CallProvider 必须与 `AthenaEngine::execute_domain` 同走 semantic entry，
        // 禁止再直达 `domains::execute_domain` 旁路 M-Graph 查询/准入。
        let residual = domain_request_residual_term(self.session, &domain);
        let domain_result = execute_domain_via_semantic_entry(self.session, domain)?;
        let projected = domain_result_symbolic_term(self.session, &domain_result);
        let missing_binding = linear_algebra_missing_binding(&domain_result);
        let mut computation = computation_from_domain(self.session, domain_result);
        if computation.symbolic_term.is_none() {
            if let Some(term) = projected {
                computation = computation.with_symbolic_term(term);
            } else if missing_binding {
                if let Some(term) = residual {
                    // 未绑定矩阵：Own 回声 Extension，状态从 Invalid 降为 Unknown。
                    computation = computation.with_symbolic_term(term);
                    computation.status = athena_types::ComputationStatus::Unknown;
                    computation.coverage = crate::runtime::results::CoverageStatus::Unsupported;
                }
            }
        }
        computation = computation.with_provenance(crate::runtime::results::ResultProvenance::call_provider(handoff.capabilities.fingerprint));
        let result_id = self.session.insert_result(computation);
        Ok(HostOutcome::Value(SlotValue::Result(result_id)))
    }

    fn read_binding(&mut self, key: SlotValue) -> Result<HostOutcome> {
        let SlotValue::Symbol(symbol) = key
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "read_key_not_symbol"),
            ));
        };
        for frame in self.frames.as_slice().iter().rev() {
            match frame.lookup(symbol) {
                Some(LocalBinding::Value(term) | LocalBinding::Unique(term)) => {
                    return Ok(HostOutcome::Value(SlotValue::Term(term)));
                }
                Some(LocalBinding::Matrix(matrix)) => {
                    let value = self.session.insert_matrix_value(matrix);
                    return Ok(HostOutcome::Value(SlotValue::Value(value)));
                }
                Some(LocalBinding::Cleared) => {
                    // Dynamic clear: do not fall through to session Own.
                    return Ok(HostOutcome::Value(SlotValue::Symbol(symbol)));
                }
                None => {}
            }
        }
        if let Some(term) = self.session.defs.binding(symbol) {
            return Ok(HostOutcome::Value(SlotValue::Term(term)));
        }
        if let Some(term) = self.session.defs.residual_binding(symbol) {
            let result_id = execute_ir_request(self.session, AthenaRequest::Term(term))?;
            let out = self.session.results.require_symbolic_term(result_id)?;
            return Ok(HostOutcome::Value(SlotValue::Term(out)));
        }
        if let Some(matrix) = self.session.defs.matrix_binding(symbol) {
            let value = self.session.insert_matrix_value(matrix);
            return Ok(HostOutcome::Value(SlotValue::Value(value)));
        }
        Ok(HostOutcome::Value(SlotValue::Symbol(symbol)))
    }

    fn write_binding(
        &mut self,
        key: SlotValue,
        value: SlotValue,
        kind: BindingKind,
        evaluation: BindingEvaluationPolicy,
    ) -> Result<HostOutcome> {
        let _ = kind;
        let SlotValue::Symbol(symbol) = key
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "write_key_not_symbol"),
            ));
        };
        let residual = !matches!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
        match value {
            SlotValue::Unit => {
                if let Some(frame) = self.frames.as_mut_vec().last_mut() {
                    frame.clear(symbol);
                }
                else {
                    self.session.defs.clear_symbol(symbol);
                }
            }
            SlotValue::Term(term) => {
                let term_ref = self.session.arena.term_ref(term).ok_or_else(|| {
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionHost")
                        .detail("reason", "write_term_out_of_range")
                })?;
                let term = self.session.arena.check_ref(term_ref)?;
                self.bind_term(symbol, term, residual);
            }
            SlotValue::Boolean(v) => {
                let term = self.session.builder().boolean(v, Default::default());
                self.bind_term(symbol, term, residual);
            }
            SlotValue::Symbol(sym) => {
                let term = self.session.builder().symbol_id(sym, Default::default());
                self.bind_term(symbol, term, residual);
            }
            SlotValue::Value(value_id) => {
                enum ValuePayload {
                    Matrix(crate::domains::linear_algebra::MatrixRef),
                    Term(TermId),
                    Boolean(bool),
                }
                let payload = match self.session.values.get(value_id) {
                    Some(crate::runtime::RuntimeValue::Matrix(matrix)) => ValuePayload::Matrix(*matrix),
                    Some(crate::runtime::RuntimeValue::SymbolicTerm(term)) => ValuePayload::Term(*term),
                    Some(crate::runtime::RuntimeValue::Boolean(v)) => ValuePayload::Boolean(*v),
                    _ => {
                        return Ok(HostOutcome::Diagnostic(
                            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                                .detail("component", "ExecutionHost")
                                .detail("reason", "write_value_payload_unsupported")
                                .detail("value", format!("{value_id:?}")),
                        ));
                    }
                };
                match payload {
                    ValuePayload::Matrix(matrix) => self.bind_matrix(symbol, matrix),
                    ValuePayload::Term(term) => self.bind_term(symbol, term, residual),
                    ValuePayload::Boolean(v) => {
                        let term = self.session.builder().boolean(v, Default::default());
                        self.bind_term(symbol, term, residual);
                    }
                }
            }
            other => {
                return Ok(HostOutcome::Diagnostic(
                    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ExecutionHost")
                        .detail("reason", "write_value_unsupported")
                        .detail("slot", format!("{other:?}")),
                ));
            }
        }
        Ok(HostOutcome::Value(SlotValue::Unit))
    }

    fn enter_scope(&mut self, parent: Option<SlotValue>) -> Result<HostOutcome> {
        let _ = parent;
        let depth = self.frames.as_slice().len() as u32;
        self.frames.as_mut_vec().push(ScopeFrame::new());
        Ok(HostOutcome::Value(SlotValue::Scope(depth)))
    }

    fn exit_scope(&mut self, scope: SlotValue) -> Result<HostOutcome> {
        let SlotValue::Scope(expected) = scope
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "exit_scope_bad_handle"),
            ));
        };
        let top = self.frames.as_slice().len().saturating_sub(1) as u32;
        if expected != top {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "exit_scope_mismatch"),
            ));
        }
        self.frames.as_mut_vec().pop();
        Ok(HostOutcome::Value(SlotValue::Unit))
    }

    fn construct_collection(&mut self, kind: CollectionKind, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut items = Vec::with_capacity(args.len());
        for slot in args {
            items.push(self.slot_as_term(*slot)?);
        }
        let span = athena_ir::TermNode::default_span();
        let term = self.session.arena.push(athena_ir::TermNode::Collection { kind, elements: items }, span);
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_index(&mut self, op: IndexAxesId, target: SlotValue) -> Result<HostOutcome> {
        let Some(axes) = self.index_axes.get(op.0 as usize).cloned()
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "index_axes_out_of_range")
                    .detail("axes", op.0),
            ));
        };
        if let SlotValue::Value(value_id) = target {
            if let Some(matrix_ref) = self.session.matrix_of_value(value_id) {
                let Some(matrix) = self.session.matrix_objects.resolve_owning(matrix_ref)
                else {
                    return Ok(HostOutcome::Diagnostic(
                        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ExecutionHost")
                            .detail("reason", "index_matrix_missing"),
                    ));
                };
                return Ok(match evaluate_index_axes_matrix(self.session, &matrix, &axes)? {
                    IndexOutcome::Term(term) => HostOutcome::Value(SlotValue::Term(term)),
                    IndexOutcome::Invalid { echo, diagnostic } => HostOutcome::SoftInvalid { value: SlotValue::Term(echo), diagnostic },
                });
            }
        }
        let cur = self.slot_as_term(target)?;
        Ok(match evaluate_index_axes(self.session, cur, &axes)? {
            IndexOutcome::Term(term) => HostOutcome::Value(SlotValue::Term(term)),
            IndexOutcome::Invalid { echo, diagnostic } => HostOutcome::SoftInvalid { value: SlotValue::Term(echo), diagnostic },
        })
    }

    fn apply_store_index(&mut self, op: IndexAxesId, target: SlotValue, value: SlotValue) -> Result<HostOutcome> {
        let Some(axes) = self.index_axes.get(op.0 as usize).cloned()
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "store_index_axes_out_of_range")
                    .detail("axes", op.0),
            ));
        };
        if let SlotValue::Value(value_id) = target {
            if let Some(matrix_ref) = self.session.matrix_of_value(value_id) {
                let Some(matrix) = self.session.matrix_objects.resolve_owning(matrix_ref)
                else {
                    return Ok(HostOutcome::Diagnostic(
                        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ExecutionHost")
                            .detail("reason", "store_index_matrix_missing"),
                    ));
                };
                let val = self.slot_as_term(value)?;
                return Ok(match store_index_axes_matrix(self.session, matrix, &axes, val)? {
                    MatrixStoreOutcome::Value(stored) => HostOutcome::Value(SlotValue::Value(stored)),
                    MatrixStoreOutcome::Invalid { echo, diagnostic } => HostOutcome::SoftInvalid { value: SlotValue::Term(echo), diagnostic },
                });
            }
        }
        let cur = self.slot_as_term(target)?;
        let val = self.slot_as_term(value)?;
        Ok(match store_index_axes(self.session, cur, &axes, val)? {
            IndexOutcome::Term(term) => HostOutcome::Value(SlotValue::Term(term)),
            IndexOutcome::Invalid { echo, diagnostic } => HostOutcome::SoftInvalid { value: SlotValue::Term(echo), diagnostic },
        })
    }

    fn apply_extension(&mut self, op: ExtensionOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        self.apply_extension_operator(athena_types::ExtensionOperatorId(op.0), args)
    }

    fn register_rule_dispatch(
        &mut self,
        head: SlotValue,
        operator: ExtensionOpId,
        pattern: SlotValue,
        replacement: SlotValue,
    ) -> Result<HostOutcome> {
        ExecutionHost::register_rule_dispatch(self, head, athena_types::ExtensionOperatorId(operator.0), pattern, replacement)
    }

    fn register_compiled_rule(&mut self, table: u32, rule: u32) -> Result<HostOutcome> {
        ExecutionHost::register_compiled_rule(self, athena_types::DispatchTableId(table), athena_types::CompiledRuleId(rule))
    }
}
