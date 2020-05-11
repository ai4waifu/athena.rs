//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期覆盖句柄级 Boolean 语义，以及可经 session 折叠的标量算术路径。
//! 完整 SSA / Term 语义仍在 [`crate::execution::reference`]。终态由 Reference 循环迁入 VM 后扩展本 host。

use athena_ir::SemanticOperator;
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};
use athena_vm::{HostOutcome, ProviderOpId, SemanticOpId, SlotValue, VmHost};

use crate::{
    execution::reference::{
        CompareOutcome, evaluate_arithmetic_terms, evaluate_compare_terms, evaluate_unary_term,
    },
    runtime::session::Session,
};

/// 执行宿主（engine 在 VM 之上 · 不拥有解释循环）。
#[derive(Debug)]
pub struct ExecutionHost<'a> {
    session: &'a mut Session,
}

impl<'a> ExecutionHost<'a> {
    /// 构造（持有 session 以便折叠需要 `TermStore` 的语义算子）。
    pub fn new(session: &'a mut Session) -> Self {
        Self { session }
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
        Ok(Self::unsupported(op))
    }

    fn call_provider(&mut self, op: ProviderOpId, args: &[SlotValue]) -> Result<HostOutcome> {
        let _ = (op, args);
        Ok(HostOutcome::Diagnostic(
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ExecutionHost")
                .detail("reason", "call_provider_deferred_to_reference")
                .detail("op", op.0),
        ))
    }
}
