//! VM 后端能力分析（显式报告，禁止用 codegen 试探冒充语义完备）。
//!
//! Living `04`：选择后端须回答「该 backend 能否完整实现语义 / 诊断 / effect /
//! 预算 / 取消 / 生命周期」，而不是「指令能否编码」。
//!
//! 先拦截已知语义缺口，再调用
//! [`crate::execution::vm_codegen::validate_vm_codegen_subset`] 做无 emit 的结构闭集校验。

use athena_ir::SemanticOperator;

use crate::execution::ir::{ExecutionModule, OperationKind, Terminator};

/// 一条阻止选择 `AthenaVm` 的能力缺口。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmCapabilityGap {
    /// 多于一个 region（当前 VM 子集仅单 region）。
    MultiRegion,
    /// 操作 / terminator 不在当前 VM 编码闭集。
    UnsupportedShape,
    /// 结构上无法编码（经无 emit 的 `validate_vm_codegen_subset`）。
    NotEncodable,
}

/// 对一份 `ExecutionModule` 的 VM 能力报告。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmCapabilityReport {
    /// 是否可安全选择 [`super::BackendKind::AthenaVm`]。
    pub supports_athena_vm: bool,
    /// 拒绝理由（可多条；空表示可走 VM）。
    pub gaps: Vec<VmCapabilityGap>,
}

impl VmCapabilityReport {
    /// 由缺口列表构造。
    pub fn from_gaps(gaps: Vec<VmCapabilityGap>) -> Self {
        Self {
            supports_athena_vm: gaps.is_empty(),
            gaps,
        }
    }

    /// 首选后端。
    pub fn preferred_backend(&self) -> super::BackendKind {
        if self.supports_athena_vm {
            super::BackendKind::AthenaVm
        } else {
            super::BackendKind::Reference
        }
    }
}

fn note(gaps: &mut Vec<VmCapabilityGap>, gap: VmCapabilityGap) {
    if !gaps.contains(&gap) {
        gaps.push(gap);
    }
}

fn scan_semantic_gaps(module: &ExecutionModule, gaps: &mut Vec<VmCapabilityGap>) {
    for region in &module.regions {
        for block in &region.blocks {
            for op in &block.operations {
                match &op.kind {
                    OperationKind::ApplySemanticOperator { operator, .. } => match *operator {
                        SemanticOperator::CollectMatches
                        | SemanticOperator::Matches
                        | SemanticOperator::ReplaceAll
                        | SemanticOperator::Rule
                        | SemanticOperator::RuleDeferred
                        | SemanticOperator::Simplify
                        | SemanticOperator::Hold => {
                            note(gaps, VmCapabilityGap::UnsupportedShape);
                        }
                        _ => {}
                    },
                    OperationKind::ApplyExtensionOperator { .. }
                    | OperationKind::RegisterRuleDispatch { .. }
                    | OperationKind::RegisterCompiledRule { .. }
                    | OperationKind::LoadInput { .. }
                    | OperationKind::MaterializeValue { .. } => {
                        note(gaps, VmCapabilityGap::UnsupportedShape);
                    }
                    _ => {}
                }
            }
            match &block.terminator {
                Terminator::Return { .. } | Terminator::Reject { .. } | Terminator::Branch { .. } => {}
                _ => note(gaps, VmCapabilityGap::UnsupportedShape),
            }
        }
    }
}

/// 分析 module 是否可由当前 `athena-vm` 路径语义完备地执行。
///
/// 不把「能生成指令」当作充分条件：先报告语义缺口，再做无 emit 的结构闭集校验。
pub fn analyze_vm_capability(module: &ExecutionModule) -> VmCapabilityReport {
    let mut gaps = Vec::new();
    if module.regions.len() != 1 {
        note(&mut gaps, VmCapabilityGap::MultiRegion);
        return VmCapabilityReport::from_gaps(gaps);
    }
    scan_semantic_gaps(module, &mut gaps);
    if !gaps.is_empty() {
        return VmCapabilityReport::from_gaps(gaps);
    }
    if crate::execution::vm_codegen::validate_vm_codegen_subset(module).is_err() {
        note(&mut gaps, VmCapabilityGap::NotEncodable);
    }
    VmCapabilityReport::from_gaps(gaps)
}
