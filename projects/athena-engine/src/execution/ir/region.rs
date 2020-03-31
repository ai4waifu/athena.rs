//! Region 将共享控制流图的基本块归为一组。

use super::{
    block::BasicBlock,
    ids::{BlockId, RegionId, SsaValueId},
    operation::OperationKind,
    terminator::Terminator,
    types::ExecutionValueType,
};

/// [`super::ExecutionModule`] 内的一个 region。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    /// Region 标识。
    pub id: RegionId,
    /// 入口基本块。
    pub entry: BlockId,
    /// 本 region 拥有的基本块。
    pub blocks: Vec<BasicBlock>,
    /// Region 成功完成时返回的值类型。
    pub result_types: Vec<ExecutionValueType>,
}

impl Region {
    /// 单基本块 region。
    pub fn from_entry_block(id: RegionId, block: BasicBlock, result_types: Vec<ExecutionValueType>) -> Self {
        let entry = block.id;
        Self { id, entry, blocks: vec![block], result_types }
    }

    /// 本 region 稠密槽表所需容量（最大 `SsaValueId.0 + 1`）。
    pub fn slot_capacity(&self) -> u32 {
        let mut max = 0u32;
        let bump = |max: &mut u32, id: SsaValueId| {
            *max = (*max).max(id.0.saturating_add(1));
        };
        let bump_edge = |max: &mut u32, edge: &super::terminator::BlockEdge| {
            for &arg in &edge.arguments {
                bump(max, arg);
            }
        };
        for block in &self.blocks {
            for param in &block.parameters {
                bump(&mut max, param.value);
            }
            for op in &block.operations {
                if let Some(result) = op.result {
                    bump(&mut max, result);
                }
                observe_kind(&op.kind, &mut max, bump);
            }
            match &block.terminator {
                Terminator::Branch { condition, then_edge, else_edge } => {
                    bump(&mut max, *condition);
                    bump_edge(&mut max, then_edge);
                    bump_edge(&mut max, else_edge);
                }
                Terminator::Switch { discriminant, cases, default } => {
                    bump(&mut max, *discriminant);
                    for (_, edge) in cases {
                        bump_edge(&mut max, edge);
                    }
                    bump_edge(&mut max, default);
                }
                Terminator::Return { values } => {
                    for &id in values {
                        bump(&mut max, id);
                    }
                }
                Terminator::Yield { values, resume } => {
                    for &id in values {
                        bump(&mut max, id);
                    }
                    bump_edge(&mut max, resume);
                }
                Terminator::Reject { .. } | Terminator::Unreachable => {}
            }
        }
        max
    }
}

fn observe_kind(kind: &OperationKind, max: &mut u32, bump: impl Fn(&mut u32, SsaValueId)) {
    match kind {
        OperationKind::LoadInput { .. }
        | OperationKind::LoadTerm { .. }
        | OperationKind::Constant { .. }
        | OperationKind::RegisterCompiledRule { .. } => {}
        OperationKind::ApplySemanticOperator { args, .. }
        | OperationKind::ApplyExtensionOperator { args, .. }
        | OperationKind::CallProvider { args, .. } => {
            for &id in args {
                bump(max, id);
            }
        }
        OperationKind::ConstructCollection { elements, .. } => {
            for &id in elements {
                bump(max, id);
            }
        }
        OperationKind::Index { target, .. } => bump(max, *target),
        OperationKind::ReadBinding { key } => bump(max, *key),
        OperationKind::WriteBinding { key, value, .. } => {
            bump(max, *key);
            bump(max, *value);
        }
        OperationKind::RegisterRuleDispatch { head, pattern, replacement, .. } => {
            bump(max, *head);
            bump(max, *pattern);
            bump(max, *replacement);
        }
        OperationKind::EnterScope { parent } => {
            if let Some(id) = parent {
                bump(max, *id);
            }
        }
        OperationKind::ExitScope { scope } => bump(max, *scope),
        OperationKind::Guard { predicate, .. } => bump(max, *predicate),
        OperationKind::MaterializeValue { source } | OperationKind::PublishResult { source } => bump(max, *source),
    }
}
