//! 把受限 Boolean `ExecutionModule` 降到 `athena-vm::VmModule`。
//!
//! engine 只做 IR→VM 投影与 `ExecutionHost`，**不**在本路径再跑 `ReferenceExecutor`。
//! 超出子集则返回诊断，由调用方回退 Reference。
//!
//! 支持：单 region · `Constant` / 受支持 `ApplySemanticOperator` ·
//! `Guard`（仅 `GuardFailure::Reject`）· `Return` / `Reject` /
//! `Branch`（含边实参 → 块参数，经 `Move` 蹦床 + `Jump`；源/目标冲突时经临时槽并行拷贝）。

use std::collections::HashMap;

use athena_types::{Diagnostic, DiagnosticCode, Result};
use athena_vm::{Instruction, MAX_HOST_ARGS, SemanticOpId, VmConstant, VmModule};

use crate::execution::ir::{
    BasicBlock, BlockEdge, BlockId, ConstantValue, ExecutionModule, GuardFailure, OperationKind, Region,
    Terminator, verify_module,
};

/// 已降级的 Boolean module。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredBooleanModule {
    /// VM 可执行模块。
    pub module: VmModule,
    /// 静态提示的结果槽（多出口时以解释器 `last_return_slot` 为准）。
    pub result_slot: u32,
}

fn supported_boolean_op(op: athena_ir::SemanticOperator) -> bool {
    use athena_ir::SemanticOperator::*;
    matches!(op, Not | And | Or | TrueQ | Equal | Unequal)
}

fn diag(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("component", "vm_lower")
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
        OperationKind::Constant { .. } | OperationKind::ApplySemanticOperator { .. } => {
            if op.result.is_none() {
                return Err(diag("lower_rejects_unit_only_op"));
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

fn lower_ops(
    module: &ExecutionModule,
    block: &BasicBlock,
    constants: &mut Vec<VmConstant>,
    instructions: &mut Vec<Instruction>,
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
                if !supported_boolean_op(*operator) {
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

/// 尝试将单 region Boolean CFG 降为 [`LoweredBooleanModule`]。
///
/// 允许：`Constant` / 受支持语义算子 · `Guard(Reject)` · `Return` / `Reject` /
/// `Branch`（边实参经 `Move` 蹦床）。
pub fn try_lower_linear_boolean_module(module: &ExecutionModule) -> Result<LoweredBooleanModule> {
    verify_module(module)?;
    if module.regions.len() != 1 {
        return Err(diag("lower_requires_single_region"));
    }
    let region = &module.regions[0];
    if region.blocks.is_empty() {
        return Err(diag("lower_requires_blocks"));
    }

    let block_pcs = layout_block_pcs(region)?;
    let mut constants = Vec::new();
    let mut instructions = Vec::new();
    let mut max_slot = 0u32;
    let mut result_slot = 0u32;
    let mut saw_return = false;

    for block in &region.blocks {
        note_block_parameters(block, &mut max_slot);
        lower_ops(module, block, &mut constants, &mut instructions, &mut max_slot)?;
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

    if !saw_return {
        return Err(diag("lower_requires_return"));
    }

    Ok(LoweredBooleanModule {
        module: VmModule::from_parts(instructions, constants, max_slot),
        result_slot,
    })
}
