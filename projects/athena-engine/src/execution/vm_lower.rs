//! 把受限 Boolean `ExecutionModule` 降到 `athena-vm::VmModule`。
//!
//! engine 只做 IR→VM 投影与 `ExecutionHost`，**不**在本路径再跑 `ReferenceExecutor`。
//! 超出子集则返回诊断，由调用方回退 Reference。
//!
//! 支持：单 region · 无块参数 · `Constant` / 受支持 `ApplySemanticOperator` ·
//! `Return` / 无边实参的 `Branch`（多块展平为 `Jump`/`Branch` PC）。

use std::collections::HashMap;

use athena_types::{Diagnostic, DiagnosticCode, Result};
use athena_vm::{Instruction, MAX_HOST_ARGS, SemanticOpId, VmConstant, VmModule};

use crate::execution::ir::{
    BasicBlock, BlockId, ConstantValue, ExecutionModule, OperationKind, Region, Terminator, verify_module,
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

fn lower_ops(
    module: &ExecutionModule,
    block: &BasicBlock,
    constants: &mut Vec<VmConstant>,
    instructions: &mut Vec<Instruction>,
    max_slot: &mut u32,
) -> Result<()> {
    for op in &block.operations {
        let Some(result) = op.result else {
            return Err(diag("lower_rejects_unit_only_op"));
        };
        bump(max_slot, result.0);
        match &op.kind {
            OperationKind::Constant { constant } => {
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

fn block_body_len(block: &BasicBlock) -> Result<u32> {
    if !block.parameters.is_empty() {
        return Err(diag("lower_rejects_block_parameters"));
    }
    let term_len = match &block.terminator {
        Terminator::Return { values } => {
            if values.len() != 1 {
                return Err(diag("lower_requires_single_return"));
            }
            1u32
        }
        Terminator::Branch { then_edge, else_edge, .. } => {
            if !then_edge.arguments.is_empty() || !else_edge.arguments.is_empty() {
                return Err(diag("lower_rejects_branch_args"));
            }
            1u32
        }
        _ => return Err(diag("lower_unsupported_terminator")),
    };
    Ok((block.operations.len() as u32).saturating_add(term_len))
}

fn layout_block_pcs(region: &Region) -> Result<HashMap<BlockId, u32>> {
    let mut pcs = HashMap::new();
    let mut cursor = 0u32;
    for block in &region.blocks {
        pcs.insert(block.id, cursor);
        cursor = cursor.saturating_add(block_body_len(block)?);
    }
    Ok(pcs)
}

/// 尝试将单 region Boolean CFG 降为 [`LoweredBooleanModule`]。
///
/// 允许：无块参数 · `Constant` / 受支持语义算子 · `Return` / 无边实参 `Branch`。
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
        lower_ops(module, block, &mut constants, &mut instructions, &mut max_slot)?;
        match &block.terminator {
            Terminator::Return { values } => {
                let slot = values[0].0;
                bump(&mut max_slot, slot);
                instructions.push(Instruction::ReturnValue { slot });
                result_slot = slot;
                saw_return = true;
            }
            Terminator::Branch {
                condition,
                then_edge,
                else_edge,
            } => {
                bump(&mut max_slot, condition.0);
                let then_pc = *block_pcs
                    .get(&then_edge.target)
                    .ok_or_else(|| diag("lower_missing_then_block"))?;
                let else_pc = *block_pcs
                    .get(&else_edge.target)
                    .ok_or_else(|| diag("lower_missing_else_block"))?;
                instructions.push(Instruction::Branch {
                    condition: condition.0,
                    then_pc,
                    else_pc,
                });
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
