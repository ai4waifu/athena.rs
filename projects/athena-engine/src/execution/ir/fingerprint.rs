//! 与源无关的 module 指纹（不是 `TermId` 下标或渲染文本）。
//!
//! 指纹必须覆盖**语义相关内容**：常量载荷、终结器、操作数、结果类型、
//! block 参数、captured roots 与 provider 描述符。仅哈希容器长度或
//! `OperationKind` discriminant 不足以作缓存 / 验证合同。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use athena_types::IndexSpec;

use super::{
    block::{BasicBlock, BlockParameter},
    effect::EffectEdge,
    exit::DeclaredExit,
    module::ExecutionModule,
    operation::{GuardFailure, Operation, OperationKind},
    terminator::{BlockEdge, Terminator},
    types::{CapturedRoot, ConstantValue, ExecutionValueType, ModuleInput, ProviderCallDescriptor},
};

/// 对 module 结构内容的稳定指纹。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleFingerprint(pub u64);

impl ModuleFingerprint {
    /// 计算结构指纹。
    ///
    /// 使用专用种子，避免默认 hasher 跨进程不稳定被静默当作缓存键。
    /// 需要跨进程稳定性的调用方，日后须换成固定字节的规范编码。
    pub fn of_module(module: &ExecutionModule) -> Self {
        let mut hasher = DefaultHasher::new();
        0x4154_4845_4e41_4558u64.hash(&mut hasher); // "ATHENAEX"
        2u64.hash(&mut hasher); // schema revision（内容字段扩充时递增）

        for input in &module.inputs {
            hash_module_input(&mut hasher, input);
        }
        for constant in &module.constants {
            hash_constant(&mut hasher, constant);
        }
        for root in &module.captured_roots {
            hash_captured_root(&mut hasher, root);
        }
        for edge in &module.effect_edges {
            hash_effect_edge(&mut hasher, edge);
        }
        for exit in &module.exits {
            hash_declared_exit(&mut hasher, exit);
        }
        for call in &module.provider_calls {
            hash_provider_call(&mut hasher, call);
        }
        for region in &module.regions {
            region.id.0.hash(&mut hasher);
            region.entry.0.hash(&mut hasher);
            for ty in &region.result_types {
                hash_value_type(&mut hasher, ty);
            }
            for block in &region.blocks {
                hash_block(&mut hasher, block);
            }
        }
        Self(hasher.finish())
    }
}

fn hash_module_input(hasher: &mut DefaultHasher, input: &ModuleInput) {
    input.id.0.hash(hasher);
    hash_value_type(hasher, &input.ty);
}

fn hash_constant(hasher: &mut DefaultHasher, constant: &ConstantValue) {
    core::mem::discriminant(constant).hash(hasher);
    match constant {
        ConstantValue::Boolean(v) => v.hash(hasher),
        ConstantValue::Symbol(s) => s.0.hash(hasher),
        ConstantValue::Term(t) => t.0.hash(hasher),
        ConstantValue::Unit => {}
    }
}

fn hash_captured_root(hasher: &mut DefaultHasher, root: &CapturedRoot) {
    core::mem::discriminant(root).hash(hasher);
    match root {
        CapturedRoot::Term(term_ref) => {
            term_ref.id.0.hash(hasher);
            term_ref.generation.hash(hasher);
            term_ref.store_id.hash(hasher);
        }
        CapturedRoot::Value(v) => v.0.hash(hasher),
        CapturedRoot::Result(r) => r.0.hash(hasher),
    }
}

fn hash_effect_edge(hasher: &mut DefaultHasher, edge: &EffectEdge) {
    edge.token.0.hash(hasher);
    edge.precedes_from.map(|t| t.0).hash(hasher);
    core::mem::discriminant(&edge.kind).hash(hasher);
}

fn hash_declared_exit(hasher: &mut DefaultHasher, exit: &DeclaredExit) {
    exit.id.0.hash(hasher);
    core::mem::discriminant(&exit.kind).hash(hasher);
    exit.continuation.map(|b| b.0).hash(hasher);
    for ty in &exit.result_types {
        hash_value_type(hasher, ty);
    }
}

fn hash_provider_call(hasher: &mut DefaultHasher, call: &ProviderCallDescriptor) {
    call.id.0.hash(hasher);
    call.operator.0.hash(hasher);
    for ty in &call.argument_types {
        hash_value_type(hasher, ty);
    }
    hash_value_type(hasher, &call.result_type);
    call.safepoint.hash(hasher);
    call.payload.map(|p| p.0).hash(hasher);
}

fn hash_block(hasher: &mut DefaultHasher, block: &BasicBlock) {
    block.id.0.hash(hasher);
    for param in &block.parameters {
        hash_block_parameter(hasher, param);
    }
    for op in &block.operations {
        hash_operation(hasher, op);
    }
    hash_terminator(hasher, &block.terminator);
}

fn hash_block_parameter(hasher: &mut DefaultHasher, param: &BlockParameter) {
    param.value.0.hash(hasher);
    hash_value_type(hasher, &param.ty);
}

fn hash_operation(hasher: &mut DefaultHasher, op: &Operation) {
    op.result.map(|v| v.0).hash(hasher);
    hash_value_type(hasher, &op.result_type);
    op.effect_in.map(|t| t.0).hash(hasher);
    op.effect_out.map(|t| t.0).hash(hasher);
    hash_operation_kind(hasher, &op.kind);
}

fn hash_operation_kind(hasher: &mut DefaultHasher, kind: &OperationKind) {
    core::mem::discriminant(kind).hash(hasher);
    match kind {
        OperationKind::LoadInput { input } => input.0.hash(hasher),
        OperationKind::LoadTerm { root } => root.0.hash(hasher),
        OperationKind::Constant { constant } => constant.0.hash(hasher),
        OperationKind::ApplySemanticOperator { operator, args } => {
            core::mem::discriminant(operator).hash(hasher);
            // `SemanticOperator` 是封闭 enum；discriminant 区分算子身份。
            for arg in args {
                arg.0.hash(hasher);
            }
        }
        OperationKind::ApplyExtensionOperator { operator, args } => {
            operator.0.hash(hasher);
            for arg in args {
                arg.0.hash(hasher);
            }
        }
        OperationKind::ConstructCollection { kind, elements } => {
            core::mem::discriminant(kind).hash(hasher);
            for el in elements {
                el.0.hash(hasher);
            }
        }
        OperationKind::Index { target, axes } => {
            target.0.hash(hasher);
            for axis in axes {
                hash_index_spec(hasher, axis);
            }
        }
        OperationKind::StoreIndex { target, axes, value } => {
            target.0.hash(hasher);
            value.0.hash(hasher);
            for axis in axes {
                hash_index_spec(hasher, axis);
            }
        }
        OperationKind::ReadBinding { key } => key.0.hash(hasher),
        OperationKind::WriteBinding { key, value, kind, evaluation } => {
            key.0.hash(hasher);
            value.0.hash(hasher);
            core::mem::discriminant(kind).hash(hasher);
            core::mem::discriminant(evaluation).hash(hasher);
        }
        OperationKind::RegisterRuleDispatch { head, operator, pattern, replacement } => {
            head.0.hash(hasher);
            operator.0.hash(hasher);
            pattern.0.hash(hasher);
            replacement.0.hash(hasher);
        }
        OperationKind::RegisterCompiledRule { table, rule } => {
            table.0.hash(hasher);
            rule.0.hash(hasher);
        }
        OperationKind::EnterScope { parent } => parent.map(|v| v.0).hash(hasher),
        OperationKind::ExitScope { scope } => scope.0.hash(hasher),
        OperationKind::CallProvider { call, args } => {
            call.0.hash(hasher);
            for arg in args {
                arg.0.hash(hasher);
            }
        }
        OperationKind::Guard { predicate, on_failure } => {
            predicate.0.hash(hasher);
            match on_failure {
                GuardFailure::Exit(exit) => {
                    0u8.hash(hasher);
                    exit.0.hash(hasher);
                }
                GuardFailure::Reject => 1u8.hash(hasher),
            }
        }
        OperationKind::MaterializeValue { source } | OperationKind::PublishResult { source } => source.0.hash(hasher),
    }
}

fn hash_index_spec(hasher: &mut DefaultHasher, axis: &IndexSpec) {
    core::mem::discriminant(axis).hash(hasher);
    match axis {
        IndexSpec::Scalar(v) => v.0.hash(hasher),
        IndexSpec::LinearColumnMajor(v) => v.0.hash(hasher),
        IndexSpec::ColumnMajorFlatten => {}
        IndexSpec::Range { start, end, step } => {
            start.0.hash(hasher);
            end.0.hash(hasher);
            step.hash(hasher);
        }
        IndexSpec::All => {}
        IndexSpec::EndRelative(offset) => offset.0.hash(hasher),
        IndexSpec::Cartesian(axes) => {
            for nested in axes {
                hash_index_spec(hasher, nested);
            }
        }
        IndexSpec::DomainSpecific(id) => id.0.hash(hasher),
    }
}

fn hash_terminator(hasher: &mut DefaultHasher, terminator: &Terminator) {
    core::mem::discriminant(terminator).hash(hasher);
    match terminator {
        Terminator::Branch { condition, then_edge, else_edge } => {
            condition.0.hash(hasher);
            hash_block_edge(hasher, then_edge);
            hash_block_edge(hasher, else_edge);
        }
        Terminator::Switch { discriminant, cases, default } => {
            discriminant.0.hash(hasher);
            for (case, edge) in cases {
                case.hash(hasher);
                hash_block_edge(hasher, edge);
            }
            hash_block_edge(hasher, default);
        }
        Terminator::Return { values } | Terminator::Yield { values, .. } => {
            for v in values {
                v.0.hash(hasher);
            }
            if let Terminator::Yield { resume, .. } = terminator {
                hash_block_edge(hasher, resume);
            }
        }
        Terminator::Reject { exit } => exit.map(|e| e.0).hash(hasher),
        Terminator::Unreachable => {}
    }
}

fn hash_block_edge(hasher: &mut DefaultHasher, edge: &BlockEdge) {
    edge.target.0.hash(hasher);
    for arg in &edge.arguments {
        arg.0.hash(hasher);
    }
}

fn hash_value_type(hasher: &mut DefaultHasher, ty: &ExecutionValueType) {
    core::mem::discriminant(ty).hash(hasher);
}
