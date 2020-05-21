//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期覆盖 Boolean、标量算术 / 比较 / 一元、`Join` / `Range`、session / 局部 binding、
//! scope 帧栈，以及运行时注入 domain 载荷的 `CallProvider`。

use athena_ir::SemanticOperator;
use athena_types::{
    BindingEvaluationPolicy, BindingKind, CollectionKind, Diagnostic, DiagnosticCode, Result, SymbolId, TermId,
};
use athena_vm::{HostOutcome, ProviderOpId, SemanticOpId, SlotValue, VmHost};

use crate::{
    api::request::AthenaRequest,
    domains::dispatch::{DomainRequest, execute_domain},
    execution::{
        LocalBinding, ScopeFrame, execute_ir_request,
        ir::ProviderCallDescriptor,
        provider::ProviderCallHandoff,
        reference::{
            CompareOutcome, domain_result_symbolic_term, evaluate_arithmetic_terms, evaluate_compare_terms,
            evaluate_join_terms, evaluate_range_terms, evaluate_size_terms, evaluate_sum_terms,
            evaluate_determinant_term, evaluate_matrix_constructor_terms, evaluate_elementwise_terms,
            evaluate_unary_term,
        },
    },
    runtime::{results::computation_from_domain, session::Session},
};

/// 执行宿主（engine 在 VM 之上 · 不拥有解释循环）。
#[derive(Debug)]
pub struct ExecutionHost<'a> {
    session: &'a mut Session,
    frames: Vec<ScopeFrame>,
    provider_calls: Vec<ProviderCallDescriptor>,
    pending_domain: Option<DomainRequest>,
}

impl<'a> ExecutionHost<'a> {
    /// 构造（持有 session；可选 domain 载荷供首条 `CallProvider` 消费）。
    pub fn new(
        session: &'a mut Session,
        provider_calls: Vec<ProviderCallDescriptor>,
        pending_domain: Option<DomainRequest>,
    ) -> Self {
        Self {
            session,
            frames: Vec::new(),
            provider_calls,
            pending_domain,
        }
    }

    fn unsupported(op: SemanticOpId) -> HostOutcome {
        HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "apply_semantic_deferred_to_reference")
                .detail("op", op.0),
        )
    }

    fn expect_boolean(args: &[SlotValue], index: usize, reason: &'static str) -> Result<core::result::Result<bool, HostOutcome>> {
        match args.get(index).copied() {
            Some(SlotValue::Boolean(v)) => Ok(Ok(v)),
            Some(_) | None => Ok(Err(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", reason),
            ))),
        }
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
        if let Some(frame) = self.frames.last_mut() {
            // 局部帧只存已物化值；残差策略在局部作用域内仍立即绑定 Value。
            let _ = residual;
            frame.bind(symbol, LocalBinding::Value(term));
            return;
        }
        if residual {
            self.session.defs.write_residual_binding(symbol, term);
        } else {
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
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let term = evaluate_sum_terms(self.session, terms)?;
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }

    fn apply_determinant(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        if args.len() != 1 {
            return Ok(Self::unsupported(SemanticOpId(SemanticOperator::Determinant.discriminant())));
        }
        let term = self.slot_as_term(args[0])?;
        let (out, diag_opt) = evaluate_determinant_term(self.session, term)?;
        if let Some(diagnostic) = diag_opt {
            return Ok(HostOutcome::Diagnostic(diagnostic));
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
}

impl VmHost for ExecutionHost<'_> {
    fn apply_semantic(&mut self, op: SemanticOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        if op.0 == SemanticOperator::Not.discriminant() {
            return match Self::expect_boolean(args, 0, "not_expects_boolean")? {
                Ok(v) => Ok(HostOutcome::Value(SlotValue::Boolean(!v))),
                Err(outcome) => Ok(outcome),
            };
        }
        if op.0 == SemanticOperator::TrueQ.discriminant() {
            return match Self::expect_boolean(args, 0, "trueq_expects_boolean")? {
                Ok(v) => Ok(HostOutcome::Value(SlotValue::Boolean(v))),
                Err(outcome) => Ok(outcome),
            };
        }
        if op.0 == SemanticOperator::And.discriminant() {
            let left = match Self::expect_boolean(args, 0, "and_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            let right = match Self::expect_boolean(args, 1, "and_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            return Ok(HostOutcome::Value(SlotValue::Boolean(left && right)));
        }
        if op.0 == SemanticOperator::Or.discriminant() {
            let left = match Self::expect_boolean(args, 0, "or_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            let right = match Self::expect_boolean(args, 1, "or_expects_boolean")? {
                Ok(v) => v,
                Err(outcome) => return Ok(outcome),
            };
            return Ok(HostOutcome::Value(SlotValue::Boolean(left || right)));
        }
        if op.0 == SemanticOperator::Equal.discriminant()
            || op.0 == SemanticOperator::Unequal.discriminant()
            || op.0 == SemanticOperator::Identical.discriminant()
        {
            if args.len() != 2 {
                return Ok(Self::unsupported(op));
            }
            match (args[0], args[1]) {
                (SlotValue::Boolean(left), SlotValue::Boolean(right)) => {
                    let eq = left == right;
                    let out = if op.0 == SemanticOperator::Unequal.discriminant() {
                        !eq
                    } else {
                        eq
                    };
                    return Ok(HostOutcome::Value(SlotValue::Boolean(out)));
                }
                (SlotValue::Term(left), SlotValue::Term(right)) => {
                    let left = self.slot_as_term(SlotValue::Term(left))?;
                    let right = self.slot_as_term(SlotValue::Term(right))?;
                    let eq = self.session.arena.structural_eq(left, right);
                    let out = if op.0 == SemanticOperator::Unequal.discriminant() {
                        !eq
                    } else {
                        eq
                    };
                    return Ok(HostOutcome::Value(SlotValue::Boolean(out)));
                }
                _ => return Ok(Self::unsupported(op)),
            }
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
        Ok(Self::unsupported(op))
    }

    fn call_provider(&mut self, op: ProviderOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = args;
        let descriptor = self
            .provider_calls
            .get(op.0 as usize)
            .cloned()
            .ok_or_else(|| {
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
        let Some(domain) = self.pending_domain.take() else {
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
        computation = computation.with_provenance(
            crate::runtime::results::ResultProvenance::call_provider(handoff.capabilities.fingerprint),
        );
        let result_id = self.session.insert_result(computation);
        Ok(HostOutcome::Value(SlotValue::Result(result_id)))
    }

    fn read_binding(&mut self, key: SlotValue) -> Result<HostOutcome> {
        let SlotValue::Symbol(symbol) = key else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "read_key_not_symbol"),
            ));
        };
        for frame in self.frames.iter().rev() {
            if let Some(LocalBinding::Value(term) | LocalBinding::Unique(term)) = frame.lookup(symbol) {
                return Ok(HostOutcome::Value(SlotValue::Term(term)));
            }
        }
        if let Some(term) = self.session.defs.binding(symbol) {
            return Ok(HostOutcome::Value(SlotValue::Term(term)));
        }
        if let Some(term) = self.session.defs.residual_binding(symbol) {
            let result_id = execute_ir_request(self.session, AthenaRequest::Term(term))?;
            let out = self
                .session
                .results
                .get(result_id)
                .and_then(|r| r.symbolic_term)
                .unwrap_or(term);
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
        let SlotValue::Symbol(symbol) = key else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "write_key_not_symbol"),
            ));
        };
        let residual = !matches!(evaluation, BindingEvaluationPolicy::EvaluateBeforeStore);
        match value {
            SlotValue::Unit => {
                if let Some(frame) = self.frames.last_mut() {
                    frame.unbind(symbol);
                } else {
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
        let depth = self.frames.len() as u32;
        self.frames.push(ScopeFrame::new());
        Ok(HostOutcome::Value(SlotValue::Scope(depth)))
    }

    fn exit_scope(&mut self, scope: SlotValue) -> Result<HostOutcome> {
        let SlotValue::Scope(expected) = scope else {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "exit_scope_bad_handle"),
            ));
        };
        let top = self.frames.len().saturating_sub(1) as u32;
        if expected != top {
            return Ok(HostOutcome::Diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ExecutionHost")
                    .detail("reason", "exit_scope_mismatch"),
            ));
        }
        self.frames.pop();
        Ok(HostOutcome::Value(SlotValue::Unit))
    }

    fn construct_collection(&mut self, kind: CollectionKind, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut items = Vec::with_capacity(args.len());
        for slot in args {
            items.push(self.slot_as_term(*slot)?);
        }
        let span = athena_ir::TermNode::default_span();
        let term = self
            .session
            .arena
            .push(athena_ir::TermNode::Collection { kind, elements: items }, span);
        Ok(HostOutcome::Value(SlotValue::Term(term)))
    }
}
