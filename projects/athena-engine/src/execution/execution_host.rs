//! [`ExecutionHost`]：engine 综合体向 `athena-vm` 提供的 [`VmHost`] 实现。
//!
//! 过渡期覆盖句柄级 Boolean 语义，以及可经 session 折叠的 `Add` 数值路径。
//! 完整 SSA / Term 语义仍在 [`crate::execution::reference`]。终态由 Reference 循环迁入 VM 后扩展本 host。

use athena_ir::SemanticOperator;
use athena_numeric::{Number, add as num_add};
use athena_types::{Diagnostic, DiagnosticCode, Result, TermId};
use athena_vm::{HostOutcome, ProviderOpId, SemanticOpId, SlotValue, VmHost};

use crate::{
    execution::{number_of, push_number, reference::fold_plus_symbolic},
    runtime::{session::Session, values::numeric_clone::clone_number},
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

    fn apply_add(&mut self, args: &[SlotValue]) -> Result<HostOutcome> {
        let mut terms = Vec::with_capacity(args.len());
        for slot in args {
            terms.push(self.slot_as_term(*slot)?);
        }
        let numbers = terms
            .iter()
            .map(|t| number_of(self.session, *t).map(clone_number))
            .collect::<Option<Vec<_>>>();
        if let Some(nums) = numbers {
            let folded = match nums.as_slice() {
                [] => Some(Number::small_int(0)),
                values => {
                    let mut acc = clone_number(&values[0]);
                    let mut ok = true;
                    for n in &values[1..] {
                        match num_add(clone_number(&acc), clone_number(n)) {
                            Ok(v) => acc = v,
                            Err(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    ok.then_some(acc)
                }
            };
            if let Some(folded) = folded {
                let term = push_number(self.session, folded);
                return Ok(HostOutcome::Value(SlotValue::Term(term)));
            }
        }
        let term = fold_plus_symbolic(self.session, terms);
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
            return self.apply_add(args);
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
