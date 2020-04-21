//! 把受限线性 Boolean `ExecutionModule` 降到 `athena-vm::VmModule`。
//!
//! 这是「解释循环归 VM」迁移的第一刀：engine 只做 IR→VM 投影与 `ExecutionHost`，
//! **不**在本路径再跑 `ReferenceExecutor`。超出子集则返回诊断，由调用方回退 Reference。

use athena_types::{Diagnostic, DiagnosticCode, Result};
use athena_vm::{Instruction, MAX_HOST_ARGS, SemanticOpId, VmConstant, VmModule};

use crate::execution::ir::{ConstantValue, ExecutionModule, OperationKind, Terminator, verify_module};

/// 已降级的线性 Boolean module。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredBooleanModule {
    /// VM 可执行模块。
    pub module: VmModule,
    /// `Return` 前结果所在绝对槽。
    pub result_slot: u32,
}

/// 当前可降级的封闭语义算子（与 [`crate::execution::execution_host::ExecutionHost`] 对齐）。
fn supported_boolean_op(op: athena_ir::SemanticOperator) -> bool {
    use athena_ir::SemanticOperator::*;
    matches!(op, Not | And | Or | TrueQ | Equal | Unequal)
}

/// 尝试将**单 region · 单基本块 · Return** 的 Boolean 线性 module 降为 [`LoweredBooleanModule`]。
///
/// 允许操作：`Constant`（Boolean / Unit / Term / Symbol）与受支持的 `ApplySemanticOperator`。
/// SSA 下标直接映射为绝对槽下标。
pub fn try_lower_linear_boolean_module(module: &ExecutionModule) -> Result<LoweredBooleanModule> {
    verify_module(module)?;
    if module.regions.len() != 1 {
        return Err(diag("lower_requires_single_region"));
    }
    let region = &module.regions[0];
    if region.blocks.len() != 1 {
        return Err(diag("lower_requires_single_block"));
    }
    let block = &region.blocks[0];
    if !block.parameters.is_empty() {
        return Err(diag("lower_rejects_block_parameters"));
    }

    let mut constants = Vec::new();
    let mut instructions = Vec::new();
    let mut max_slot = 0u32;

    let bump = |max: &mut u32, id: u32| {
        *max = (*max).max(id.saturating_add(1));
    };

    for op in &block.operations {
        let Some(result) = op.result else {
            return Err(diag("lower_rejects_unit_only_op"));
        };
        bump(&mut max_slot, result.0);
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
                    bump(&mut max_slot, arg.0);
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

    let result_slot = match &block.terminator {
        Terminator::Return { values } => {
            if values.len() != 1 {
                return Err(diag("lower_requires_single_return"));
            }
            bump(&mut max_slot, values[0].0);
            instructions.push(Instruction::Return);
            values[0].0
        }
        _ => return Err(diag("lower_requires_return_terminator")),
    };

    Ok(LoweredBooleanModule {
        module: VmModule::from_parts(instructions, constants, max_slot),
        result_slot,
    })
}

fn diag(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("component", "vm_lower")
        .detail("reason", reason)
}
