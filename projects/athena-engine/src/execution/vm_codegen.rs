//! 把已验证 CFG 子集 `ExecutionModule` 降到 `athena-vm::VmModule`。
//!
//! engine 只做 IR→VM 投影与 `ExecutionHost`，**不**在本路径再跑 `ReferenceExecutor`。
//! 超出子集则返回诊断。后端由 [`crate::execution::backend::select_execution_backend`]
//! **事先**显式选择，禁止执行失败后再静默回退 Reference。
//!
//! 支持：单 region · `LoadTerm` / `Constant` / 受支持 `ApplySemanticOperator`
//! （Boolean + 标量算术 / 比较 / 一元 / `Join` / `Range` / `Size` / `Sum` / `Product` / `Determinant` /
//! `Zeros` / `Ones` / `Eye` / 逐元算术 / `Map` / `Apply` / `ApplyHead` / `Function` /
//! `Rule` / `ReplaceAll` / `Matches` / `CollectMatches` / `Simplify`）· `ApplyExtension` ·
//! `RegisterRuleDispatch` / `RegisterCompiledRule` · `ReadBinding` / `WriteBinding` ·
//! `EnterScope` / `ExitScope` · `Guard`（仅 `GuardFailure::Reject`）· `Return` / `Reject` /
//! `Branch`（含边实参 → 块参数，经 `Move` 蹦床 + `Jump`；源/目标冲突时经临时槽并行拷贝）。
//!
//! 含：`CallProvider` / `PublishResult` / `ConstructCollection`（元素数 ≤ `MAX_HOST_ARGS`）/
//! `Index`（轴规格登记在 codegen 表）。
//! 不含：多 region（仍由显式 backend 选择走 Reference）。

use std::collections::HashMap;

use athena_types::{Diagnostic, DiagnosticCode, IndexSpec, Result};
use athena_vm::{IndexAxesId, Instruction, MAX_HOST_ARGS, ProviderOpId, SemanticOpId, VmConstant, VmModule};

use crate::execution::ir::{
    BasicBlock, BlockEdge, BlockId, CapturedRoot, ConstantValue, ExecutionModule, GuardFailure, OperationKind,
    Region, Terminator, verify_module,
};

/// Verified CFG 子集的 VM codegen 产物（布局 / 调用约定，不改变语义）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmCodegenArtifact {
    /// VM 可执行模块。
    pub module: VmModule,
    /// 静态提示的结果槽（多出口时以解释器 `last_return_slot` 为准）。
    pub result_slot: u32,
    /// `Index` 指令引用的轴规格表（与 [`IndexAxesId`] 对齐）。
    pub index_axes: Vec<Vec<IndexSpec>>,
}

fn supported_semantic_op(op: athena_ir::SemanticOperator) -> bool {
    use athena_ir::SemanticOperator::*;
    matches!(
        op,
        Not | And | Or | TrueQ | Equal | Unequal | Identical | Add | Multiply | Subtract | Negate | Divide | Power
            | Less | Greater | LessEqual | GreaterEqual | Abs | Factorial | Sqrt | Length | First | Rest
            | Join | Range | Size | Sum | Product | Determinant | Zeros | Ones | Eye
            | ElementwiseMultiply | ElementwiseDivide | ElementwisePower
            | Map | Apply | ApplyHead | Function | Rule | RuleDeferred | ReplaceAll | Matches | CollectMatches
            | Simplify | Unary(_)
    )
}

fn diag(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("component", "vm_codegen")
        .detail("reason", reason)
}

fn bump(max: &mut u32, id: u32) {
    *max = (*max).max(id.saturating_add(1));
}

fn edge_moves_interfere(
    params: &[crate::execution::ir::BlockParameter],
    arguments: &[crate::execution::ir::SsaValueId],
) -> bool {
    let mut dsts = HashMap::new();
    for param in params {
        dsts.insert(param.value.0, ());
    }
    arguments.iter().any(|arg| dsts.contains_key(&arg.0))
}

fn edge_trampoline_len(region: &Region, edge: &BlockEdge) -> Result<u32> {
    if edge.arguments.is_empty() {
        return Ok(0);
    }
    let target = region
        .blocks
        .iter()
        .find(|b| b.id == edge.target)
        .ok_or_else(|| diag("lower_missing_edge_target"))?;
    if target.parameters.len() != edge.arguments.len() {
        return Err(diag("lower_edge_arity_mismatch"));
    }
    let n = edge.arguments.len() as u32;
    let moves = if edge_moves_interfere(&target.parameters, &edge.arguments) {
        n.saturating_mul(2)
    } else {
        n
    };
    Ok(moves.saturating_add(1))
}

fn op_instruction_len(op: &crate::execution::ir::Operation) -> Result<u32> {
    match &op.kind {
        OperationKind::LoadTerm { .. }
        | OperationKind::Constant { .. }
        | OperationKind::ApplySemanticOperator { .. }
        | OperationKind::ApplyExtensionOperator { .. }
        | OperationKind::ReadBinding { .. }
        | OperationKind::WriteBinding { .. }
        | OperationKind::EnterScope { .. }
        | OperationKind::CallProvider { .. }
        | OperationKind::PublishResult { .. }
        | OperationKind::ConstructCollection { .. }
        | OperationKind::Index { .. }
        | OperationKind::RegisterRuleDispatch { .. }
        | OperationKind::RegisterCompiledRule { .. } => {
            if op.result.is_none() {
                return Err(diag("lower_rejects_unit_only_op"));
            }
            Ok(1)
        }
        OperationKind::ExitScope { .. } => {
            if op.result.is_some() {
                return Err(diag("lower_rejects_exit_scope_result"));
            }
            Ok(1)
        }
        OperationKind::Guard { on_failure, .. } => {
            if !matches!(on_failure, GuardFailure::Reject) {
                return Err(diag("lower_unsupported_guard_failure"));
            }
            if op.result.is_some() {
                return Err(diag("lower_rejects_guard_result"));
            }
            Ok(1)
        }
        _ => Err(diag("lower_unsupported_operation")),
    }
}

/// 校验单条操作是否落在当前 VM codegen 闭集（不 emit）。
fn validate_op(module: &ExecutionModule, op: &crate::execution::ir::Operation) -> Result<()> {
    let _ = op_instruction_len(op)?;
    match &op.kind {
        OperationKind::LoadTerm { root } => {
            let captured = module
                .captured_roots
                .get(root.0 as usize)
                .ok_or_else(|| diag("lower_missing_root"))?;
            match captured {
                CapturedRoot::Term(_) => Ok(()),
                CapturedRoot::Value(_) | CapturedRoot::Result(_) => Err(diag("lower_root_not_term")),
            }
        }
        OperationKind::Constant { constant } => {
            let _ = module
                .constants
                .get(constant.0 as usize)
                .ok_or_else(|| diag("lower_missing_constant"))?;
            Ok(())
        }
        OperationKind::ApplySemanticOperator { operator, args } => {
            if !supported_semantic_op(*operator) {
                return Err(diag("lower_unsupported_semantic_op"));
            }
            if args.len() > MAX_HOST_ARGS {
                return Err(diag("lower_argc_overflow"));
            }
            Ok(())
        }
        OperationKind::ApplyExtensionOperator { args, .. } => {
            if args.len() > MAX_HOST_ARGS {
                return Err(diag("lower_argc_overflow"));
            }
            Ok(())
        }
        OperationKind::CallProvider { args, .. } => {
            if args.len() > MAX_HOST_ARGS {
                return Err(diag("lower_argc_overflow"));
            }
            Ok(())
        }
        OperationKind::ConstructCollection { elements, .. } => {
            if elements.len() > MAX_HOST_ARGS {
                return Err(diag("lower_collection_argc_overflow"));
            }
            Ok(())
        }
        OperationKind::Guard { .. }
        | OperationKind::ReadBinding { .. }
        | OperationKind::WriteBinding { .. }
        | OperationKind::EnterScope { .. }
        | OperationKind::ExitScope { .. }
        | OperationKind::PublishResult { .. }
        | OperationKind::Index { .. }
        | OperationKind::RegisterRuleDispatch { .. }
        | OperationKind::RegisterCompiledRule { .. } => Ok(()),
        _ => Err(diag("lower_unsupported_operation")),
    }
}

/// 结构闭集校验：单 region · 操作 / terminator 可编码 · 至少一处 `Return`。
///
/// **不**生成指令。供 [`crate::execution::backend::analyze_vm_capability`] 与
/// [`try_lower_verified_cfg_module`] 共用，避免选择阶段靠 emit 试探。
pub fn validate_vm_codegen_subset(module: &ExecutionModule) -> Result<()> {
    verify_module(module)?;
    if module.regions.len() != 1 {
        return Err(diag("lower_requires_single_region"));
    }
    let region = &module.regions[0];
    if region.blocks.is_empty() {
        return Err(diag("lower_requires_blocks"));
    }
    let _ = layout_block_pcs(region)?;
    let mut saw_return = false;
    for block in &region.blocks {
        for op in &block.operations {
            validate_op(module, op)?;
        }
        match &block.terminator {
            Terminator::Return { values } => {
                if values.len() != 1 {
                    return Err(diag("lower_requires_single_return"));
                }
                saw_return = true;
            }
            Terminator::Reject { .. } | Terminator::Branch { .. } => {}
            _ => return Err(diag("lower_unsupported_terminator")),
        }
    }
    if !saw_return {
        return Err(diag("lower_requires_return"));
    }
    Ok(())
}

fn lower_ops(
    module: &ExecutionModule,
    block: &BasicBlock,
    constants: &mut Vec<VmConstant>,
    instructions: &mut Vec<Instruction>,
    index_axes: &mut Vec<Vec<IndexSpec>>,
    max_slot: &mut u32,
) -> Result<()> {
    for op in &block.operations {
        match &op.kind {
            OperationKind::Guard { predicate, on_failure } => {
                let _ = op_instruction_len(op)?;
                bump(max_slot, predicate.0);
                let _ = on_failure;
                instructions.push(Instruction::Guard {
                    predicate: predicate.0,
                });
            }
            OperationKind::LoadTerm { root } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                let captured = module
                    .captured_roots
                    .get(root.0 as usize)
                    .ok_or_else(|| diag("lower_missing_root"))?;
                let term = match captured {
                    CapturedRoot::Term(term_ref) => term_ref.id,
                    CapturedRoot::Value(_) | CapturedRoot::Result(_) => {
                        return Err(diag("lower_root_not_term"));
                    }
                };
                let const_index = constants.len() as u32;
                constants.push(VmConstant::Term(term));
                instructions.push(Instruction::LoadConstant {
                    dst: result.0,
                    constant: const_index,
                });
            }
            OperationKind::Constant { constant } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                let value = module
                    .constants
                    .get(constant.0 as usize)
                    .ok_or_else(|| diag("lower_missing_constant"))?;
                let vm_const = match value {
                    ConstantValue::Boolean(v) => VmConstant::Boolean(*v),
                    ConstantValue::Unit => VmConstant::Unit,
                    ConstantValue::Term(term) => VmConstant::Term(*term),
                    ConstantValue::Symbol(symbol) => VmConstant::Symbol(*symbol),
                };
                let const_index = constants.len() as u32;
                constants.push(vm_const);
                instructions.push(Instruction::LoadConstant {
                    dst: result.0,
                    constant: const_index,
                });
            }
            OperationKind::ApplySemanticOperator { operator, args } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                if !supported_semantic_op(*operator) {
                    return Err(diag("lower_unsupported_semantic_op"));
                }
                if args.len() > MAX_HOST_ARGS {
                    return Err(diag("lower_argc_overflow"));
                }
                let mut packed = [0u32; MAX_HOST_ARGS];
                for (i, arg) in args.iter().enumerate() {
                    bump(max_slot, arg.0);
                    packed[i] = arg.0;
                }
                instructions.push(Instruction::ApplySemantic {
                    dst: result.0,
                    op: SemanticOpId(operator.discriminant()),
                    argc: args.len() as u8,
                    args: packed,
                });
            }
            OperationKind::ApplyExtensionOperator { operator, args } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                if args.len() > MAX_HOST_ARGS {
                    return Err(diag("lower_argc_overflow"));
                }
                let mut packed = [0u32; MAX_HOST_ARGS];
                for (i, arg) in args.iter().enumerate() {
                    bump(max_slot, arg.0);
                    packed[i] = arg.0;
                }
                instructions.push(Instruction::ApplyExtension {
                    dst: result.0,
                    op: athena_vm::ExtensionOpId(operator.0),
                    argc: args.len() as u8,
                    args: packed,
                });
            }
            OperationKind::RegisterRuleDispatch {
                head,
                operator,
                pattern,
                replacement,
            } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                bump(max_slot, head.0);
                bump(max_slot, pattern.0);
                bump(max_slot, replacement.0);
                instructions.push(Instruction::RegisterRuleDispatch {
                    dst: result.0,
                    head: head.0,
                    operator: athena_vm::ExtensionOpId(operator.0),
                    pattern: pattern.0,
                    replacement: replacement.0,
                });
            }
            OperationKind::RegisterCompiledRule { table, rule } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                instructions.push(Instruction::RegisterCompiledRule {
                    dst: result.0,
                    table: table.0,
                    rule: rule.0,
                });
            }
            OperationKind::ReadBinding { key } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                bump(max_slot, key.0);
                instructions.push(Instruction::ReadBinding {
                    dst: result.0,
                    key: key.0,
                });
            }
            OperationKind::WriteBinding {
                key,
                value,
                kind,
                evaluation,
            } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                bump(max_slot, key.0);
                bump(max_slot, value.0);
                instructions.push(Instruction::WriteBinding {
                    dst: result.0,
                    key: key.0,
                    value: value.0,
                    kind: *kind,
                    evaluation: *evaluation,
                });
            }
            OperationKind::EnterScope { parent } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                let parent_slot = match parent {
                    Some(p) => {
                        bump(max_slot, p.0);
                        Some(p.0)
                    }
                    None => None,
                };
                instructions.push(Instruction::EnterScope {
                    dst: result.0,
                    parent: parent_slot,
                });
            }
            OperationKind::ExitScope { scope } => {
                let _ = op_instruction_len(op)?;
                bump(max_slot, scope.0);
                instructions.push(Instruction::ExitScope { scope: scope.0 });
            }
            OperationKind::CallProvider { call, args } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                if args.len() > MAX_HOST_ARGS {
                    return Err(diag("lower_argc_overflow"));
                }
                let mut packed = [0u32; MAX_HOST_ARGS];
                for (i, arg) in args.iter().enumerate() {
                    bump(max_slot, arg.0);
                    packed[i] = arg.0;
                }
                instructions.push(Instruction::CallProvider {
                    dst: result.0,
                    op: ProviderOpId(call.0),
                    argc: args.len() as u8,
                    args: packed,
                });
            }
            OperationKind::PublishResult { source } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                bump(max_slot, source.0);
                instructions.push(Instruction::Move {
                    dst: result.0,
                    src: source.0,
                });
            }
            OperationKind::ConstructCollection { kind, elements } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                if elements.len() > MAX_HOST_ARGS {
                    return Err(diag("lower_collection_argc_overflow"));
                }
                let mut packed = [0u32; MAX_HOST_ARGS];
                for (i, arg) in elements.iter().enumerate() {
                    bump(max_slot, arg.0);
                    packed[i] = arg.0;
                }
                instructions.push(Instruction::ConstructCollection {
                    dst: result.0,
                    kind: *kind,
                    argc: elements.len() as u8,
                    args: packed,
                });
            }
            OperationKind::Index { target, axes } => {
                let result = op.result.ok_or_else(|| diag("lower_rejects_unit_only_op"))?;
                bump(max_slot, result.0);
                bump(max_slot, target.0);
                let axes_id = IndexAxesId(index_axes.len() as u32);
                index_axes.push(axes.clone());
                instructions.push(Instruction::Index {
                    dst: result.0,
                    target: target.0,
                    axes: axes_id,
                });
            }
            _ => return Err(diag("lower_unsupported_operation")),
        }
    }
    Ok(())
}

fn block_body_len(region: &Region, block: &BasicBlock) -> Result<u32> {
    let mut ops_len = 0u32;
    for op in &block.operations {
        ops_len = ops_len.saturating_add(op_instruction_len(op)?);
    }
    let term_len = match &block.terminator {
        Terminator::Return { values } => {
            if values.len() != 1 {
                return Err(diag("lower_requires_single_return"));
            }
            1u32
        }
        Terminator::Reject { .. } => 1u32,
        Terminator::Branch { then_edge, else_edge, .. } => 1u32
            .saturating_add(edge_trampoline_len(region, then_edge)?)
            .saturating_add(edge_trampoline_len(region, else_edge)?),
        _ => return Err(diag("lower_unsupported_terminator")),
    };
    Ok(ops_len.saturating_add(term_len))
}

fn layout_block_pcs(region: &Region) -> Result<HashMap<BlockId, u32>> {
    let mut pcs = HashMap::new();
    let mut cursor = 0u32;
    for block in &region.blocks {
        pcs.insert(block.id, cursor);
        cursor = cursor.saturating_add(block_body_len(region, block)?);
    }
    Ok(pcs)
}

fn note_block_parameters(block: &BasicBlock, max_slot: &mut u32) {
    for param in &block.parameters {
        bump(max_slot, param.value.0);
    }
}

fn emit_edge_trampoline(
    region: &Region,
    edge: &BlockEdge,
    block_pcs: &HashMap<BlockId, u32>,
    instructions: &mut Vec<Instruction>,
    max_slot: &mut u32,
) -> Result<()> {
    if edge.arguments.is_empty() {
        return Ok(());
    }
    let target = region
        .blocks
        .iter()
        .find(|b| b.id == edge.target)
        .ok_or_else(|| diag("lower_missing_edge_target"))?;
    if target.parameters.len() != edge.arguments.len() {
        return Err(diag("lower_edge_arity_mismatch"));
    }
    if edge_moves_interfere(&target.parameters, &edge.arguments) {
        let mut temps = Vec::with_capacity(edge.arguments.len());
        for arg in &edge.arguments {
            bump(max_slot, arg.0);
            let tmp = *max_slot;
            *max_slot = max_slot.saturating_add(1);
            instructions.push(Instruction::Move {
                dst: tmp,
                src: arg.0,
            });
            temps.push(tmp);
        }
        for (param, tmp) in target.parameters.iter().zip(temps.iter()) {
            bump(max_slot, param.value.0);
            instructions.push(Instruction::Move {
                dst: param.value.0,
                src: *tmp,
            });
        }
    } else {
        for (param, arg) in target.parameters.iter().zip(edge.arguments.iter()) {
            bump(max_slot, param.value.0);
            bump(max_slot, arg.0);
            instructions.push(Instruction::Move {
                dst: param.value.0,
                src: arg.0,
            });
        }
    }
    let target_pc = *block_pcs
        .get(&edge.target)
        .ok_or_else(|| diag("lower_missing_edge_target_pc"))?;
    instructions.push(Instruction::Jump { target: target_pc });
    Ok(())
}

fn emit_branch(
    region: &Region,
    condition: u32,
    then_edge: &BlockEdge,
    else_edge: &BlockEdge,
    block_pcs: &HashMap<BlockId, u32>,
    instructions: &mut Vec<Instruction>,
    max_slot: &mut u32,
) -> Result<()> {
    bump(max_slot, condition);
    let after_branch = (instructions.len() as u32).saturating_add(1);
    let then_tramp = edge_trampoline_len(region, then_edge)?;
    let then_pc = if then_tramp == 0 {
        *block_pcs
            .get(&then_edge.target)
            .ok_or_else(|| diag("lower_missing_then_block"))?
    } else {
        after_branch
    };
    let else_pc = if edge_trampoline_len(region, else_edge)? == 0 {
        *block_pcs
            .get(&else_edge.target)
            .ok_or_else(|| diag("lower_missing_else_block"))?
    } else {
        after_branch.saturating_add(then_tramp)
    };
    instructions.push(Instruction::Branch {
        condition,
        then_pc,
        else_pc,
    });
    emit_edge_trampoline(region, then_edge, block_pcs, instructions, max_slot)?;
    emit_edge_trampoline(region, else_edge, block_pcs, instructions, max_slot)?;
    Ok(())
}

/// 尝试将单 region verified CFG 子集编码为 [`VmCodegenArtifact`]。
///
/// 允许：`LoadTerm` / `Constant` / 受支持语义算子 · `ReadBinding` / `WriteBinding` ·
/// `EnterScope` / `ExitScope` · `CallProvider` / `PublishResult` · `ConstructCollection` ·
/// `Index` · `Guard(Reject)` · `Return` / `Reject` / `Branch`（边实参经 `Move` 蹦床）。
pub fn try_lower_verified_cfg_module(module: &ExecutionModule) -> Result<VmCodegenArtifact> {
    validate_vm_codegen_subset(module)?;
    let region = &module.regions[0];

    let block_pcs = layout_block_pcs(region)?;
    let mut constants = Vec::new();
    let mut instructions = Vec::new();
    let mut index_axes = Vec::new();
    let mut max_slot = 0u32;
    let mut result_slot = 0u32;
    let mut saw_return = false;

    for block in &region.blocks {
        note_block_parameters(block, &mut max_slot);
        lower_ops(
            module,
            block,
            &mut constants,
            &mut instructions,
            &mut index_axes,
            &mut max_slot,
        )?;
        match &block.terminator {
            Terminator::Return { values } => {
                let slot = values[0].0;
                bump(&mut max_slot, slot);
                instructions.push(Instruction::ReturnValue { slot });
                result_slot = slot;
                saw_return = true;
            }
            Terminator::Reject { .. } => {
                instructions.push(Instruction::Reject);
            }
            Terminator::Branch {
                condition,
                then_edge,
                else_edge,
            } => {
                emit_branch(
                    region,
                    condition.0,
                    then_edge,
                    else_edge,
                    &block_pcs,
                    &mut instructions,
                    &mut max_slot,
                )?;
            }
            _ => return Err(diag("lower_unsupported_terminator")),
        }
    }

    debug_assert!(saw_return);

    Ok(VmCodegenArtifact {
        module: VmModule::from_parts(instructions, constants, max_slot),
        result_slot,
        index_axes,
    })
}
