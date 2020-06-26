//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期覆盖 Boolean、标量算术 / 比较 / 一元、`Join` / `Range`、session / 局部 binding、
//! scope 帧栈、`Index`，以及运行时注入 domain 载荷的 `CallProvider`。

use athena_ir::SemanticOperator;
use athena_numeric::compare as num_compare;
use athena_types::{BindingEvaluationPolicy, BindingKind, CollectionKind, Diagnostic, DiagnosticCode, IndexSpec, Result, SymbolId, TermId};
use athena_vm::{ExtensionOpId, HostOutcome, IndexAxesId, ProviderOpId, SemanticOpId, SlotValue, VmHost};

use crate::{
    api::request::AthenaRequest,
    domains::dispatch::{DomainRequest, execute_domain},
    execution::{
        LocalBinding, ScopeFrame, execute_ir_request,
        ir::ProviderCallDescriptor,
        number_of,
        provider::ProviderCallHandoff,
        push_semantic,
        reference::{
            CompareOutcome, IndexOutcome, domain_result_symbolic_term, evaluate_apply_head_terms, evaluate_apply_terms,
            evaluate_arithmetic_terms, evaluate_collect_matches_terms, evaluate_compare_terms, evaluate_determinant_term,
            evaluate_elementwise_terms, evaluate_extension_apply_terms, evaluate_index_axes, evaluate_join_terms, evaluate_map_terms,
            evaluate_matches_terms, evaluate_matrix_constructor_terms, evaluate_product_iterator_terms, evaluate_product_terms,
            evaluate_range_terms, evaluate_replace_all_terms, evaluate_rule_terms, evaluate_simplify_terms, evaluate_size_terms,
            evaluate_special_unary_terms, evaluate_sum_iterator_terms, evaluate_sum_terms, evaluate_unary_term, slot_as_boolean_like,
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
    pending_domain: Option<DomainRequest>,
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
    /// 构造（持有 session；可选 domain 载荷供首条 `CallProvider` 消费）。
    pub fn new(
        session: &'a mut Session,
        provider_calls: Vec<ProviderCallDescriptor>,
        pending_domain: Option<DomainRequest>,
        index_axes: Vec<Vec<IndexSpec>>,
    ) -> Self {
        Self { session, frames: FrameStorage::Owned(Vec::new()), provider_calls, pending_domain, index_axes }
    }

    /// 与 Reference / 外部作用域帧栈共享同一 `frames`（收窄第二套循环）。
    pub fn with_shared_frames(
        session: &'a mut Session,
        frames: &'a mut Vec<ScopeFrame>,
        provider_calls: Vec<ProviderCallDescriptor>,
        pending_domain: Option<DomainRequest>,
        index_axes: Vec<Vec<IndexSpec>>,
    ) -> Self {
        Self { session, frames: FrameStorage::Borrowed(frames), provider_calls, pending_domain, index_axes }
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
            let term = evaluate_special_unary_terms(self.session, op, terms)?;
            return Ok(HostOutcome::Value(SlotValue::Term(term)));
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

    fn apply_join(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_join_terms(self.session, terms)?;
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
        let term = evaluate_special_unary_terms(self.session, op, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
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
        if op.0 == SemanticOperator::Join.discriminant() {
            return self.apply_join(args);
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
        if op.0 == SemanticOperator::ElementwiseMultiply.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwiseMultiply, args);
        }
        if op.0 == SemanticOperator::ElementwiseDivide.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwiseDivide, args);
        }
        if op.0 == SemanticOperator::ElementwisePower.discriminant() {
            return self.apply_elementwise(SemanticOperator::ElementwisePower, args);
        }
        if op.0 == SemanticOperator::Map.discriminant() {
            return self.apply_map(args);
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
        let handoff = ProviderCallHandoff::from_descriptor(descriptor);
        let Some(domain) = self.pending_domain.take()
        else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "provider_domain_missing")
                    .detail("op", op.0),
            ));
        };
        let domain_result = execute_domain(self.session, domain)?;
        let projected = domain_result_symbolic_term(self.session, &domain_result);
        let mut computation = computation_from_domain(self.session, domain_result);
        if computation.symbolic_term.is_none() {
            if let Some(term) = projected {
                computation = computation.with_symbolic_term(term);
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
            if let Some(LocalBinding::Value(term) | LocalBinding::Unique(term)) = frame.lookup(symbol) {
                return Ok(HostOutcome::Value(SlotValue::Term(term)));
            }
        }
        if let Some(term) = self.session.defs.binding(symbol) {
            return Ok(HostOutcome::Value(SlotValue::Term(term)));
        }
        if let Some(term) = self.session.defs.residual_binding(symbol) {
            let result_id = execute_ir_request(self.session, AthenaRequest::Term(term))?;
            let out = self.session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term);
            return Ok(HostOutcome::Value(SlotValue::Term(out)));
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
                    frame.unbind(symbol);
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
        let cur = self.slot_as_term(target)?;
        Ok(match evaluate_index_axes(self.session, cur, &axes)? {
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
