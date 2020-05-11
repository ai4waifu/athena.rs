//! `ReferenceExecutor` — 过渡期 **host adapter**（暂住 engine）。
//!
//! 终态：SSA 解释循环归属 [`athena_vm`]；本模块实现 [`athena_vm::VmHost`]（语义 / provider /
//! Session 句柄）并把 `VmExit` / 槽结果映射为 `ComputationResult`。当前仍执行 SSA 块（无操作数栈），
//! 并已使用 `athena_vm::SlotTable`。**不是**与 VM 并列的第二套解释器，也不是旧栈式 VM 包装。

mod helpers;
mod ops;

pub(crate) use self::helpers::{
    CompareOutcome, evaluate_arithmetic_terms, evaluate_compare_terms, evaluate_unary_term,
};

use self::helpers::*;

use std::{cmp::Ordering, collections::HashMap};

use athena_ir::SemanticOperator;
use athena_numeric::{
    Integer, Number, Rational, abs as num_abs, add as num_add, compare as num_compare, div as num_div, factorial as num_factorial,
    mul as num_mul, pow as num_pow, sqrt as num_sqrt, to_f64_lossy as num_to_f64_lossy,
};
use athena_types::{ComputationStatus, Diagnostic, DiagnosticCode, Result, ResultId, SymbolId, TermId};
use athena_vm::{ExecutionLease, SlotTable, VmConfig};

use crate::{
    api::request::AthenaRequest,
    domains::{
        dispatch::{DomainRequest, execute_domain},
        linear_algebra::{MatrixEntry, MatrixValue, SolveDisposition, det_bareiss, solve_exact},
    },
    execution::{
        compiler::ExecutionCompiler,
        environment::{LocalBinding, ScopeFrame},
        ir::{BlockId, CapturedRoot, ConstantValue, ExecutionModule, GuardFailure, OperationKind, RegionId, SsaValueId, Terminator, verify_module},
        number_of, push_extension, push_number, push_semantic,
    },
    runtime::{
        results::{ComputationResult, CoverageStatus, ResultProvenance, computation_from_domain},
        session::Session,
        values::{
            arena::push_list,
            numeric_clone::{clone_integer, clone_number, clone_rational},
        },
    },
};

/// 供一致性测试与确定性回放共用的语义预言机后端。
#[derive(Debug, Default)]
pub struct ReferenceExecutor {}

/// SSA 运行时槽（`athena-vm` 句柄；不与 `TermId` 共用标识域）。
pub(crate) use athena_vm::SlotValue as Slot;

#[derive(Debug)]
pub(crate) enum IndexStep {
    Next(TermId),
    Residual,
    Invalid { echo: TermId, diagnostic: Diagnostic },
}

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
        let (returned, unsupported, unevaluated, invalid) = self.eval_region(session, module, region_id, &mut provider, config)?;
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
                    SemanticOperator::Less | SemanticOperator::Greater | SemanticOperator::LessEqual | SemanticOperator::GreaterEqual => {
                        self.eval_compare_chain(session, op, args, slots)
                    }
                    SemanticOperator::Add
                    | SemanticOperator::Multiply
                    | SemanticOperator::Subtract
                    | SemanticOperator::Negate
                    | SemanticOperator::Divide
                    | SemanticOperator::Power => self.eval_arithmetic(session, op, args, slots),
                    SemanticOperator::ElementwiseMultiply | SemanticOperator::ElementwiseDivide | SemanticOperator::ElementwisePower => {
                        self.eval_dot_arithmetic(session, op, args, slots)
                    }
                    SemanticOperator::Abs
                    | SemanticOperator::Length
                    | SemanticOperator::First
                    | SemanticOperator::Rest
                    | SemanticOperator::Factorial
                    | SemanticOperator::Sqrt => self.eval_unary_term_op(session, op, args, slots),
                    SemanticOperator::Join => self.eval_join(session, args, slots),
                    SemanticOperator::Range => self.eval_range(session, args, slots),
                    SemanticOperator::Apply => self.eval_apply(session, args, slots),
                    SemanticOperator::ApplyHead => self.eval_application_form(session, args, slots),
                    SemanticOperator::Size => self.eval_size(session, args, slots),
                    SemanticOperator::Sum => self.eval_sum(session, args, slots),
                    SemanticOperator::Product => self.eval_product(session, args, slots),
                    SemanticOperator::Determinant => self.eval_det(session, args, slots, invalid),
                    SemanticOperator::Map => self.eval_map(session, args, slots),
                    SemanticOperator::Zeros | SemanticOperator::Ones | SemanticOperator::Eye => {
                        self.eval_matrix_constructor(session, op, args, slots)
                    }
                    SemanticOperator::Rule | SemanticOperator::RuleDeferred => self.eval_rule(session, op, args, slots),
                    SemanticOperator::ReplaceAll => self.eval_replace_all(session, args, slots),
                    SemanticOperator::CollectMatches => self.eval_collect_matches(session, args, slots),
                    SemanticOperator::Matches => self.eval_matches(session, args, slots),
                    SemanticOperator::Simplify => self.eval_simplify(session, args, slots),
                    SemanticOperator::Hold
                    | SemanticOperator::Function
                    | SemanticOperator::Unary(_)
                    | SemanticOperator::PolyGamma
                    | SemanticOperator::Differentiate
                    | SemanticOperator::Integrate
                    | SemanticOperator::Limit
                    | SemanticOperator::Series
                    | SemanticOperator::LaurentSeries
                    | SemanticOperator::Asymptotic
                    | SemanticOperator::Residue
                    | SemanticOperator::DSolve
                    | SemanticOperator::LaplaceTransform
                    | SemanticOperator::FourierTransform
                    | SemanticOperator::ZTransform
                    | SemanticOperator::Divergence
                    | SemanticOperator::Curl => self.eval_residual_semantic(session, op, args, slots),
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
                let mut items = Vec::with_capacity(elements.len());
                for id in elements {
                    let slot = slots.get(id.0).ok_or_else(|| diag("collection_element_undefined"))?;
                    items.push(self.slot_as_term(session, slot)?);
                }
                let span = athena_ir::TermNode::default_span();
                Ok(Slot::Term(session.arena.push(athena_ir::TermNode::Collection { kind: *kind, elements: items }, span)))
            }
            OperationKind::Index { target, axes } => self.eval_index(session, *target, axes, slots, invalid),
            OperationKind::EnterScope { .. } => {
                let depth = frames.len() as u32;
                frames.push(ScopeFrame::new());
                Ok(Slot::Scope(depth))
            }
            OperationKind::ExitScope { scope } => {
                let expected = match slots.get(scope.0) {
                    Some(Slot::Scope(depth)) => depth,
                    _ => return Err(diag("exit_scope_bad_handle")),
                };
                let top = frames.len().saturating_sub(1) as u32;
                if expected != top {
                    return Err(diag("exit_scope_mismatch"));
                }
                frames.pop();
                Ok(Slot::Unit)
            }
            OperationKind::WriteBinding { key, value, kind: _, evaluation } => {
                let symbol = match slots.get(key.0) {
                    Some(Slot::Symbol(symbol)) => symbol,
                    _ => return Err(diag("write_key_not_symbol")),
                };
                let residual = !matches!(evaluation, athena_types::BindingEvaluationPolicy::EvaluateBeforeStore);
                match slots.get(value.0) {
                    Some(Slot::Unit) => {
                        if let Some(frame) = frames.last_mut() {
                            frame.unbind(symbol);
                        }
                        else {
                            // 经 `SymbolId`→`ExtensionOperatorId` 映射清除自有扩展规则。
                            session.defs.clear_symbol(symbol);
                        }
                    }
                    Some(Slot::Term(term)) => {
                        if residual {
                            if let Some(frame) = frames.last_mut() {
                                frame.bind(symbol, LocalBinding::Value(term));
                            }
                            else {
                                session.defs.write_residual_binding(symbol, term);
                            }
                        }
                        else if let Some(frame) = frames.last_mut() {
                            frame.bind(symbol, LocalBinding::Value(term));
                        }
                        else {
                            session.defs.write_binding(symbol, term);
                        }
                    }
                    Some(Slot::Boolean(v)) => {
                        let term = session.builder().boolean(v, Default::default());
                        if residual {
                            if let Some(frame) = frames.last_mut() {
                                frame.bind(symbol, LocalBinding::Value(term));
                            }
                            else {
                                session.defs.write_residual_binding(symbol, term);
                            }
                        }
                        else if let Some(frame) = frames.last_mut() {
                            frame.bind(symbol, LocalBinding::Value(term));
                        }
                        else {
                            session.defs.write_binding(symbol, term);
                        }
                    }
                    _ => return Err(diag("write_value_unsupported")),
                }
                Ok(Slot::Unit)
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
                let symbol = match slots.get(key.0) {
                    Some(Slot::Symbol(symbol)) => symbol,
                    _ => return Err(diag("read_key_not_symbol")),
                };
                for frame in frames.iter().rev() {
                    if let Some(LocalBinding::Value(term) | LocalBinding::Unique(term)) = frame.lookup(symbol) {
                        return Ok(Slot::Term(term));
                    }
                }
                if let Some(term) = session.defs.binding(symbol) {
                    return Ok(Slot::Term(term));
                }
                if let Some(term) = session.defs.residual_binding(symbol) {
                    // 读取时求值残差绑定。
                    let module = ExecutionCompiler::new().compile(session, &AthenaRequest::Term(term))?;
                    let result_id = self.execute(session, &module, None)?;
                    let out = session.results.get(result_id).and_then(|r| r.symbolic_term).unwrap_or(term);
                    return Ok(Slot::Term(out));
                }
                Ok(Slot::Term(session.builder().symbol_id(symbol, Default::default())))
            }
            OperationKind::CallProvider { call, .. } => {
                let descriptor = module.provider_calls.get(call.0 as usize).ok_or_else(|| diag("missing_provider_call"))?.clone();
                if descriptor.id != *call {
                    return Err(diag("provider_call_id_mismatch"));
                }
                let handoff = crate::execution::provider::ProviderCallHandoff::from_descriptor(descriptor);
                match provider.take() {
                    Some(domain) => {
                        let domain_result = execute_domain(session, domain)?;
                        let projected = helpers::domain_result_symbolic_term(session, &domain_result);
                        let mut computation = computation_from_domain(session, domain_result);
                        if computation.symbolic_term.is_none() {
                            if let Some(term) = projected {
                                computation = computation.with_symbolic_term(term);
                            }
                        }
                        computation = computation
                            .with_provenance(crate::runtime::results::ResultProvenance::call_provider(handoff.capabilities.fingerprint));
                        Ok(Slot::Result(session.insert_result(computation)))
                    }
                    None => {
                        let _ = handoff;
                        *unsupported = true;
                        Ok(Slot::Unit)
                    }
                }
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
