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

    pub(crate) fn eval_simplify(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() != 1 {
            return Err(diag("semantic_operator_arity"));
        }
        let term = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let evaluated = match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term)) {
            Ok(module) => {
                let result_id = self.execute(session, &module, None)?;
                session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term)
            }
            Err(_) => term,
        };
        if let Some(one) = try_pythagorean_session(session, evaluated) {
            return Ok(Slot::Term(one));
        }
        Ok(Slot::Term(evaluated))
    }

    pub(crate) fn eval_rule(
        &self,
        session: &mut Session,
        op: SemanticOperator,
        args: &[SsaValueId],
        slots: &SlotTable,
    ) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let lhs = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rhs = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        Ok(Slot::Term(evaluate_rule_terms(session, op, lhs, rhs)?))
    }

    pub(crate) fn eval_replace_all(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let expr = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let rules_term = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        Ok(Slot::Term(evaluate_replace_all_terms(session, expr, rules_term)?))
    }

    /// `CollectMatches[list, pat]` — 按 pattern 过滤列表元素。
    pub(crate) fn eval_collect_matches(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let list = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let pat = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        Ok(Slot::Term(evaluate_collect_matches_terms(session, list, pat)?))
    }

    /// `Matches[expr, pat]` — 布尔 pattern 测试。
    pub(crate) fn eval_matches(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let expr = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let pat = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        Ok(Slot::Boolean(evaluate_matches_terms(session, expr, pat)?))
    }

    pub(crate) fn eval_apply(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.len() != 2 {
            return Err(diag("semantic_operator_arity"));
        }
        let head = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let second = self.slot_as_term(session, slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let items = match session.arena.get(second) {
            Some(athena_ir::TermNode::Collection { elements: items, .. }) => items.clone(),
            _ => return Ok(Slot::Term(push_semantic(session, SemanticOperator::Apply, vec![head, second]))),
        };
        let app = rebuild_application(session, head, items);
        match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(app)) {
            Ok(module) => {
                let result_id = self.execute(session, &module, None)?;
                let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(app);
                Ok(Slot::Term(term))
            }
            Err(_) => Ok(Slot::Term(app)),
        }
    }

    /// `Application[head, args…]` — 应用 `Function[var, body]` 或符号头。
    pub(crate) fn eval_application_form(&self, session: &mut Session, args: &[SsaValueId], slots: &SlotTable) -> Result<Slot> {
        if args.is_empty() {
            return Err(diag("semantic_operator_arity"));
        }
        let head = self.slot_as_term(session, slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?)?;
        let mut call_args = Vec::with_capacity(args.len().saturating_sub(1));
        for id in &args[1..] {
            let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
            call_args.push(self.slot_as_term(session, slot)?);
        }
        // `Function[var, body][arg…]` → 替换并重新求值。
        // 纯 `Function[body]` 需要方言 lowering 的 `AnonymousArgument`（不是字符串 Slot）。
        if let Some(athena_ir::TermNode::Application { head: op, arguments }) = session.arena.get(head) {
            if matches!(*op, ApplicationHead::Semantic(SemanticOperator::Function)) && call_args.len() == 1 {
                let arguments = arguments.clone();
                if let [var, body] = arguments.as_slice() {
                    if let Some(athena_ir::TermNode::Atom(athena_ir::Atom::Symbol(sym))) = session.arena.get(*var) {
                        let sym = *sym;
                        let instantiated = crate::execution::builtins::patterns::substitute_symbol(session, *body, sym, call_args[0]);
                        match ExecutionCompiler::new().compile(session, &AthenaRequest::Term(instantiated)) {
                            Ok(module) => {
                                let result_id = self.execute(session, &module, None)?;
                                let term = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(instantiated);
                                return Ok(Slot::Term(term));
                            }
                            Err(_) => return Ok(Slot::Term(instantiated)),
                        }
                    }
                }
            }
        }
        // 禁止裸符号经显示名 intern 成扩展算子；保留 typed `ApplyHead` 残差。
        let mut wrapped = Vec::with_capacity(call_args.len() + 1);
        wrapped.push(head);
        wrapped.extend(call_args);
        Ok(Slot::Term(push_semantic(session, SemanticOperator::ApplyHead, wrapped)))
    }
}
