//! `ReferenceExecutor` — 过渡期 **host adapter**（暂住 engine）。
//!
//! 终态：SSA 解释循环归属 [`athena_vm`]；本模块实现 [`athena_vm::VmHost`]（语义 / provider /
//! Session 句柄）并把 `VmExit` / 槽结果映射为 `ComputationResult`。当前仍执行 SSA 块（无操作数栈），
//! 并已使用 `athena_vm::SlotTable`。可委托的语义算子经 [`host_bridge`] 走 [`crate::execution::execution_host::ExecutionHost`]。
//! **不是**与 VM 并列的第二套解释器，也不是旧栈式 VM 包装。

mod helpers;
mod host_bridge;
mod ops;

pub(crate) use self::helpers::{
    CompareOutcome, IndexOutcome, domain_result_symbolic_term, evaluate_arithmetic_terms, evaluate_compare_terms,
    evaluate_join_terms, evaluate_range_terms, evaluate_size_terms, evaluate_sum_terms, evaluate_unary_term,
    evaluate_determinant_term, evaluate_matrix_constructor_terms, evaluate_elementwise_terms, evaluate_index_axes,
};

use self::helpers::*;
use self::host_bridge::{
    delegate_call_provider, host_outcome_to_slot, host_outcome_to_slot_capturing_invalid, host_with_shared_frames,
    host_with_shared_frames_and_axes, try_delegate_semantic_to_host,
};

use std::{cmp::Ordering, collections::HashMap};

use athena_ir::SemanticOperator;
use athena_numeric::compare as num_compare;
use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, Result, ResultId};
use athena_vm::{ExecutionLease, SlotTable, VmConfig, VmHost};

use crate::{
    domains::dispatch::DomainRequest,
    execution::{
        environment::ScopeFrame,
        ir::{BlockId, CapturedRoot, ConstantValue, ExecutionModule, GuardFailure, OperationKind, RegionId, SsaValueId, Terminator, verify_module},
        number_of,
    },
    runtime::{
        results::{ComputationResult, CoverageStatus, ResultProvenance},
        session::Session,
        values::numeric_clone::clone_number,
    },
};

/// 供一致性测试与确定性回放共用的语义预言机后端。
#[derive(Debug, Default)]
pub struct ReferenceExecutor {}

/// SSA 运行时槽（`athena-vm` 句柄；不与 `TermId` 共用标识域）。
pub(crate) use athena_vm::SlotValue as Slot;

impl ReferenceExecutor {
    /// 创建 reference 执行器。
    pub fn new() -> Self {
        Self {}
    }

    /// 在给定 Session / 运行时上下文中执行已校验 module。
    ///
    /// 当 `domain` 为 `Some` 时，首条 `CallProvider` 边运行 `execute_domain`
    /// 并返回该物化的 `ResultId`（IR 形态的 Goal 路径）。
    pub fn execute(&self, session: &mut Session, module: &ExecutionModule, domain: Option<DomainRequest>) -> Result<ResultId> {
        let config = crate::execution::vm::vm_config_from_session(session);
        self.execute_configured(session, module, domain, &config)
    }

    /// 带 [`VmConfig`]（cancel / budget / gc_mode）的执行入口。
    ///
    /// 持有 [`ExecutionLease`] 覆盖整次解释，Drop 时注销本执行登记的 root。
    pub fn execute_configured(
        &self,
        session: &mut Session,
        module: &ExecutionModule,
        domain: Option<DomainRequest>,
        config: &VmConfig,
    ) -> Result<ResultId> {
        verify_module(module)?;
        let mut lease = ExecutionLease::new(session.heap().clone());
        crate::execution::vm::pin_module_terms(&mut lease, &session.arena, module)?;
        let region_id = module.entry_region().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "ReferenceExecutor")
                .detail("reason", "missing_entry_region")
        })?;
        let mut provider = domain;
        let (returned, unsupported, unevaluated, invalid) =
            self.eval_region(session, module, region_id, &mut provider, config, &mut lease)?;
        drop(lease);
        if let Some(Slot::Result(result_id)) = returned {
            return Ok(result_id);
        }
        let term = match returned {
            Some(Slot::Term(term)) => term,
            Some(Slot::Boolean(value)) => session.builder().boolean(value, Default::default()),
            Some(Slot::Symbol(symbol)) => session.builder().symbol_id(symbol, Default::default()),
            Some(Slot::Scope(_)) | Some(Slot::Unit) | Some(Slot::Result(_)) | Some(Slot::Value(_)) | Some(Slot::Empty) | None => {
                session.builder().null(Default::default())
            }
        };
        let value = session.insert_symbolic_value(term);
        let mut result = if let Some(diagnostic) = invalid {
            ComputationResult::with_status(ComputationStatus::Invalid, CoverageStatus::Partial).with_diagnostic(diagnostic)
        }
        else if unsupported {
            ComputationResult::with_status(ComputationStatus::Unknown, CoverageStatus::Unsupported).with_diagnostic(
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "CallProvider")
                    .detail("status", "provider_bootstrap_unsupported"),
            )
        }
        else if unevaluated {
            ComputationResult::with_status(ComputationStatus::Unknown, CoverageStatus::Partial)
        }
        else {
            ComputationResult::with_status(ComputationStatus::Exact, CoverageStatus::Full)
        };
        result = result.with_value(value).with_symbolic_term(term).with_provenance(ResultProvenance::kind("ExecutionIR"));
        Ok(session.insert_result(result))
    }

    fn eval_region(
        &self,
        session: &mut Session,
        module: &ExecutionModule,
        region_id: RegionId,
        provider: &mut Option<DomainRequest>,
        config: &VmConfig,
        lease: &mut ExecutionLease,
    ) -> Result<(Option<Slot>, bool, bool, Option<Diagnostic>)> {
        let region = module.regions.iter().find(|r| r.id == region_id).ok_or_else(|| diag("missing_region"))?;
        let mut block_id = region.entry;
        let mut slots = SlotTable::with_capacity(region.slot_capacity() as usize);
        let mut frames: Vec<ScopeFrame> = Vec::new();
        let mut unsupported = false;
        let mut unevaluated = false;
        let mut invalid: Option<Diagnostic> = None;
        let mut block_visits: HashMap<BlockId, u32> = HashMap::new();
        let mut steps = 0u64;
        // 引导：允许有限循环回边；限制每块访问次数。
        for _ in 0..region.blocks.len().saturating_mul(64).max(64) {
            if config.cancellation.is_cancelled() {
                return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("component", "ReferenceExecutor")
                    .detail("reason", "cancelled"));
            }
            let visits = block_visits.entry(block_id).or_insert(0);
            *visits = visits.saturating_add(1);
            if *visits > 32 {
                // 热块预算耗尽 — 以 Unit 残差退出。
                return Ok((Some(Slot::Unit), unsupported, unevaluated, invalid));
            }
            let block = region.blocks.iter().find(|b| b.id == block_id).ok_or_else(|| diag("missing_block"))?;
            for op in &block.operations {
                steps = steps.saturating_add(1);
                if let Some(max) = config.max_steps {
                    if steps > max {
                        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                            .detail("component", "ReferenceExecutor")
                            .detail("reason", "budget_exceeded"));
                    }
                }
                if config.cancellation.is_cancelled() {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ReferenceExecutor")
                        .detail("reason", "cancelled"));
                }
                let produced = self.eval_operation(
                    session,
                    module,
                    &slots,
                    &mut frames,
                    &mut unsupported,
                    &mut unevaluated,
                    &mut invalid,
                    provider,
                    lease,
                    config,
                    &op.kind,
                )?;
                if let Some(result) = op.result {
                    slots.set(result.0, produced);
                }
            }
            match &block.terminator {
                Terminator::Return { values } => {
                    if values.is_empty() {
                        return Ok((Some(Slot::Unit), unsupported, unevaluated, invalid));
                    }
                    let first = values[0];
                    return Ok((Some(slots.get(first.0).ok_or_else(|| diag("return_undefined"))?), unsupported, unevaluated, invalid));
                }
                Terminator::Branch { condition, then_edge, else_edge } => {
                    let pred = match slots.get(condition.0).ok_or_else(|| diag("branch_undefined"))? {
                        Slot::Boolean(v) => Ok(v),
                        Slot::Term(term) => coerce_branch_predicate(session, term),
                        _ => Err(Diagnostic::new(DiagnosticCode::NonBooleanCondition)
                            .detail("component", "ReferenceExecutor")
                            .detail("reason", "branch_not_boolean")),
                    };
                    let pred = match pred {
                        Ok(v) => v,
                        Err(diagnostic) => {
                            // 类似 VM 的软失败：Invalid + 未求值（Null 残差）。
                            return Ok((Some(Slot::Unit), unsupported, true, Some(diagnostic)));
                        }
                    };
                    let edge = if pred { then_edge } else { else_edge };
                    self.bind_edge_args(&mut slots, region, edge.target, &edge.arguments)?;
                    block_id = edge.target;
                }
                Terminator::Reject { .. } => {
                    return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                        .detail("component", "ReferenceExecutor")
                        .detail("reason", "rejected"));
                }
                Terminator::Unreachable => return Err(diag("unreachable")),
                Terminator::Switch { .. } | Terminator::Yield { .. } => {
                    return Err(diag("terminator_not_implemented"));
                }
            }
        }
        Err(diag("block_visit_budget_exceeded"))
    }

    fn bind_edge_args(
        &self,
        slots: &mut SlotTable,
        region: &crate::execution::ir::Region,
        target: BlockId,
        arguments: &[SsaValueId],
    ) -> Result<()> {
        let block = region.blocks.iter().find(|b| b.id == target).ok_or_else(|| diag("edge_target_missing"))?;
        if block.parameters.len() != arguments.len() {
            return Err(diag("edge_arity_mismatch"));
        }
        for (param, arg) in block.parameters.iter().zip(arguments.iter()) {
            let value = slots.get(arg.0).ok_or_else(|| diag("edge_arg_undefined"))?;
            slots.set(param.value.0, value);
        }
        Ok(())
    }

    fn eval_operation(
        &self,
        session: &mut Session,
        module: &ExecutionModule,
        slots: &SlotTable,
        frames: &mut Vec<ScopeFrame>,
        unsupported: &mut bool,
        unevaluated: &mut bool,
        invalid: &mut Option<Diagnostic>,
        provider: &mut Option<DomainRequest>,
        lease: &mut ExecutionLease,
        config: &VmConfig,
        kind: &OperationKind,
    ) -> Result<Slot> {
        match kind {
            OperationKind::LoadTerm { root } => {
                let captured = module.captured_roots.get(root.0 as usize).ok_or_else(|| diag("missing_root"))?;
                match captured {
                    CapturedRoot::Term(term_ref) => {
                        let id = session.arena.check_ref(*term_ref)?;
                        Ok(Slot::Term(id))
                    }
                    CapturedRoot::Value(_) | CapturedRoot::Result(_) => Err(diag("root_not_term")),
                }
            }
            OperationKind::Constant { constant } => {
                let value = module.constants.get(constant.0 as usize).ok_or_else(|| diag("missing_constant"))?;
                Ok(match value {
                    ConstantValue::Boolean(v) => Slot::Boolean(*v),
                    ConstantValue::Symbol(symbol) => Slot::Symbol(*symbol),
                    ConstantValue::Term(term) => {
                        let term_ref = session.arena.term_ref(*term).ok_or_else(|| diag("term_out_of_range"))?;
                        let id = session.arena.check_ref(term_ref)?;
                        Slot::Term(id)
                    }
                    ConstantValue::Unit => Slot::Unit,
                })
            }
            OperationKind::ApplySemanticOperator { operator, args } => {
                let op = *operator;
                let arg_slots: Vec<Slot> = args
                    .iter()
                    .map(|id| slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined")))
                    .collect::<Result<Vec<_>>>()?;
                if let Some(slot) = try_delegate_semantic_to_host(session, op, &arg_slots, invalid)? {
                    return Ok(slot);
                }
                match op {
                    SemanticOperator::Not | SemanticOperator::And | SemanticOperator::Or | SemanticOperator::TrueQ => {
                        let bools: Vec<Option<bool>> = args
                            .iter()
                            .map(|id| {
                                let slot = slots.get(id.0).ok_or_else(|| diag("semantic_arg_undefined"))?;
                                Ok(slot_as_boolean_like(session, slot))
                            })
                            .collect::<Result<Vec<_>>>()?;
                        if bools.iter().any(|b| b.is_none()) {
                            return self.eval_residual_semantic(session, op, args, slots);
                        }
                        let bools: Vec<bool> = bools.into_iter().map(|b| b.expect("checked")).collect();
                        let result = match (op, bools.as_slice()) {
                            (SemanticOperator::Not, [a]) => !*a,
                            (SemanticOperator::TrueQ, [a]) => *a,
                            (SemanticOperator::And, values) => values.iter().copied().all(|v| v),
                            (SemanticOperator::Or, values) => values.iter().copied().any(|v| v),
                            _ => return Err(diag("semantic_operator_arity")),
                        };
                        Ok(Slot::Boolean(result))
                    }
                    SemanticOperator::Identical | SemanticOperator::Equal | SemanticOperator::Unequal => {
                        if args.len() != 2 {
                            return Err(diag("semantic_operator_arity"));
                        }
                        let left = slots.get(args[0].0).ok_or_else(|| diag("semantic_arg_undefined"))?;
                        let right = slots.get(args[1].0).ok_or_else(|| diag("semantic_arg_undefined"))?;
                        // `Identical` 是结构比较。`Equal` / `Unequal` 仅在可比较
                        // 原子上判定；符号残差保持为 `Equal[...]`（不静默成 `False`）。
                        if op == SemanticOperator::Identical {
                            let same = match (left, right) {
                                (Slot::Boolean(a), Slot::Boolean(b)) => a == b,
                                (Slot::Symbol(a), Slot::Symbol(b)) => a == b,
                                (Slot::Term(a), Slot::Term(b)) => session.arena.structural_eq(a, b),
                                (Slot::Unit, Slot::Unit) => true,
                                _ => false,
                            };
                            return Ok(Slot::Boolean(same));
                        }
                        match (left, right) {
                            (Slot::Boolean(a), Slot::Boolean(b)) => {
                                Ok(Slot::Boolean(if op == SemanticOperator::Unequal { a != b } else { a == b }))
                            }
                            (Slot::Symbol(a), Slot::Symbol(b)) => {
                                Ok(Slot::Boolean(if op == SemanticOperator::Unequal { a != b } else { a == b }))
                            }
                            (Slot::Unit, Slot::Unit) => Ok(Slot::Boolean(op != SemanticOperator::Unequal)),
                            (Slot::Term(a), Slot::Term(b)) => {
                                if session.arena.structural_eq(a, b) {
                                    return Ok(Slot::Boolean(op != SemanticOperator::Unequal));
                                }
                                let na = number_of(session, a).map(clone_number);
                                let nb = number_of(session, b).map(clone_number);
                                if let (Some(left_n), Some(right_n)) = (na, nb) {
                                    let ord = num_compare(&left_n, &right_n).ok_or_else(|| diag("compare_failed"))?;
                                    let eq = ord == Ordering::Equal;
                                    return Ok(Slot::Boolean(if op == SemanticOperator::Unequal { !eq } else { eq }));
                                }
                                self.eval_residual_semantic(session, op, args, slots)
                            }
                            _ => self.eval_residual_semantic(session, op, args, slots),
                        }
                    }
                    // 二元 iterator `Sum` / `Product` 等仍本地；host 已覆盖的算术·比较·一元·结构算子不再重复。
                    SemanticOperator::Sum => self.eval_sum(session, args, slots),
                    SemanticOperator::Product => self.eval_product(session, args, slots),
                    SemanticOperator::Apply => self.eval_apply(session, args, slots),
                    SemanticOperator::ApplyHead => self.eval_application_form(session, args, slots),
                    SemanticOperator::Map => self.eval_map(session, args, slots),
                    SemanticOperator::Rule | SemanticOperator::RuleDeferred => self.eval_rule(session, op, args, slots),
                    SemanticOperator::ReplaceAll => self.eval_replace_all(session, args, slots),
                    SemanticOperator::CollectMatches => self.eval_collect_matches(session, args, slots),
                    SemanticOperator::Matches => self.eval_matches(session, args, slots),
                    SemanticOperator::Simplify => self.eval_simplify(session, args, slots),
                    _ => self.eval_residual_semantic(session, op, args, slots),
                }
            }
            OperationKind::ApplyExtensionOperator { operator, args } => {
                // 残差重建仅使用 `ExtensionOperatorId`。
                let op = *operator;
                if let Some(slot) = self.try_apply_down_values(session, op, args, slots)? {
                    return Ok(slot);
                }
                *unevaluated = true;
                self.eval_residual_app(session, op, args, slots)
            }
            OperationKind::ConstructCollection { kind, elements } => {
                let element_slots: Vec<Slot> = elements
                    .iter()
                    .map(|id| slots.get(id.0).ok_or_else(|| diag("collection_element_undefined")))
                    .collect::<Result<Vec<_>>>()?;
                let mut host = host_with_shared_frames(session, frames, Vec::new(), None);
                host_outcome_to_slot(host.construct_collection(*kind, &element_slots)?)
            }
            OperationKind::Index { target, axes } => {
                let target_slot = slots.get(target.0).ok_or_else(|| diag("index_target_undefined"))?;
                let mut host = host_with_shared_frames_and_axes(session, frames, vec![axes.clone()]);
                host_outcome_to_slot_capturing_invalid(
                    host.apply_index(athena_vm::IndexAxesId(0), target_slot)?,
                    invalid,
                )
            }
            OperationKind::EnterScope { .. } => {
                let mut host = host_with_shared_frames(session, frames, Vec::new(), None);
                host_outcome_to_slot(host.enter_scope(None)?)
            }
            OperationKind::ExitScope { scope } => {
                let scope_slot = slots.get(scope.0).ok_or_else(|| diag("exit_scope_bad_handle"))?;
                let mut host = host_with_shared_frames(session, frames, Vec::new(), None);
                host_outcome_to_slot(host.exit_scope(scope_slot)?)
            }
            OperationKind::WriteBinding { key, value, kind, evaluation } => {
                let key_slot = slots.get(key.0).ok_or_else(|| diag("write_key_not_symbol"))?;
                let value_slot = slots.get(value.0).ok_or_else(|| diag("write_value_unsupported"))?;
                let mut host = host_with_shared_frames(session, frames, Vec::new(), None);
                host_outcome_to_slot(host.write_binding(key_slot, value_slot, *kind, *evaluation)?)
            }
            OperationKind::RegisterRuleDispatch { head, operator, pattern, replacement } => {
                let symbol = match slots.get(head.0) {
                    Some(Slot::Symbol(symbol)) => symbol,
                    _ => return Err(diag("write_key_not_symbol")),
                };
                let pattern_term = match slots.get(pattern.0) {
                    Some(Slot::Term(term)) => term,
                    _ => return Err(diag("write_pattern_not_term")),
                };
                let value_term = match slots.get(replacement.0) {
                    Some(Slot::Term(term)) => term,
                    _ => return Err(diag("write_value_unsupported")),
                };
                // 仅从项做结构编译。通配须经 API 以类型化 `TermPattern` 传入。
                let compiled = crate::execution::builtins::patterns::structural_pattern_from_term(session, pattern_term);
                // `ExtensionOperatorId` 在编译期封闭。执行时不 intern 显示名。
                session.defs.register_extension_rule_for_symbol(symbol, *operator, compiled, value_term);
                Ok(Slot::Unit)
            }
            OperationKind::RegisterCompiledRule { table, rule } => {
                let Some((pattern, replacement)) =
                    session.compiled_rules.get(*rule).map(|(pattern, replacement)| (pattern.owning_copy(), *replacement))
                else {
                    return Err(diag("compiled_rule_missing"));
                };
                session.defs.append_rule(*table, pattern, replacement);
                Ok(Slot::Unit)
            }
            OperationKind::ReadBinding { key } => {
                let key_slot = slots.get(key.0).ok_or_else(|| diag("read_key_not_symbol"))?;
                let mut host = host_with_shared_frames(session, frames, Vec::new(), None);
                let slot = host_outcome_to_slot(host.read_binding(key_slot)?)?;
                // Host 未绑定符号返回 `Symbol` 槽；Reference 合同物化为 Term。
                Ok(match slot {
                    Slot::Symbol(symbol) => Slot::Term(session.builder().symbol_id(symbol, Default::default())),
                    other => other,
                })
            }
            OperationKind::CallProvider { call, .. } => {
                // 与 VM `CallProvider` 同合同：进入 lease safepoint，再委托 `ExecutionHost`。
                lease.enter_safepoint(config.gc_mode)?;
                let pending = provider.take();
                let (slot, soft_unsupported) = delegate_call_provider(session, frames, module, pending, *call)?;
                if soft_unsupported {
                    *unsupported = true;
                }
                Ok(slot)
            }
            OperationKind::PublishResult { source } => Ok(slots.get(source.0).ok_or_else(|| diag("publish_source_undefined"))?),
            OperationKind::Guard { predicate, on_failure } => {
                let pred = match slots.get(predicate.0).ok_or_else(|| diag("guard_undefined"))? {
                    Slot::Boolean(v) => Ok(v),
                    Slot::Term(term) => coerce_branch_predicate(session, term),
                    _ => Err(Diagnostic::new(DiagnosticCode::NonBooleanCondition)
                        .detail("component", "ReferenceExecutor")
                        .detail("reason", "guard_not_boolean")),
                }?;
                match on_failure {
                    GuardFailure::Reject => {
                        if !pred {
                            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                                .detail("component", "ReferenceExecutor")
                                .detail("reason", "rejected"));
                        }
                        Ok(Slot::Unit)
                    }
                    GuardFailure::Exit(_) => Err(diag("guard_exit_not_implemented")),
                }
            }
            OperationKind::LoadInput { .. } | OperationKind::MaterializeValue { .. } => {
                Err(diag("operation_not_implemented"))
            }
        }
    }
}
