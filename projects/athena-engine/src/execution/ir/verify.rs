//! [`ExecutionModule`](super::ExecutionModule) 的结构校验器。
//!
//! 覆盖定义唯一性、同块 SSA 顺序、控制边、effect-token
//! 配对 / 链成员关系、guard 出口表，以及指纹重算。
//! 完整支配关系证明留待后续加固。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::{
    ModuleFingerprint,
    ids::{BlockId, RegionId, SsaValueId},
    module::ExecutionModule,
    operation::{GuardFailure, OperationKind},
    terminator::Terminator,
};

use std::collections::{HashMap, HashSet};

/// 校验 module 在结构上可被后端接受。
pub fn verify_module(module: &ExecutionModule) -> Result<()> {
    if module.regions.is_empty() {
        return Err(diag("empty_regions"));
    }
    verify_effect_edges(module)?;

    let mut defined = HashSet::new();
    let mut block_index: HashMap<(RegionId, BlockId), usize> = HashMap::new();
    let effect_tokens: HashSet<_> = module.effect_edges.iter().map(|e| e.token).collect();
    let exit_ids: HashSet<_> = module.exits.iter().map(|e| e.id).collect();

    for region in &module.regions {
        for (idx, block) in region.blocks.iter().enumerate() {
            let key = (region.id, block.id);
            if block_index.insert(key, idx).is_some() {
                return Err(diag("duplicate_block_id"));
            }
        }
        if !block_index.contains_key(&(region.id, region.entry)) {
            return Err(diag("missing_entry_block"));
        }

        for block in &region.blocks {
            let mut local_defs = HashSet::new();
            for param in &block.parameters {
                if !defined.insert(param.value) {
                    return Err(diag("duplicate_ssa_definition"));
                }
                local_defs.insert(param.value);
            }
            for op in &block.operations {
                for used in operands_of(&op.kind) {
                    if !local_defs.contains(&used) && !defined.contains(&used) {
                        // 跨块使用须经块参数接线；引导阶段仅允许
                        // 在 module 遍历中已先定义的值
                        //（前驱日后须将其作为块参数传入）。
                        if !defined.contains(&used) {
                            return Err(diag("use_before_def"));
                        }
                    }
                }
                if let Some(result) = op.result {
                    if !defined.insert(result) {
                        return Err(diag("duplicate_ssa_definition"));
                    }
                    local_defs.insert(result);
                }
                match (op.effect_in, op.effect_out) {
                    (None, None) => {}
                    (Some(ein), Some(eout)) => {
                        if !effect_tokens.contains(&ein) || !effect_tokens.contains(&eout) {
                            return Err(diag("effect_token_unknown"));
                        }
                    }
                    _ => return Err(diag("effect_token_pair_mismatch")),
                }
                if let OperationKind::Guard { on_failure: GuardFailure::Exit(exit), .. } = &op.kind {
                    if !exit_ids.contains(exit) {
                        return Err(diag("guard_exit_unknown"));
                    }
                }
            }
            verify_terminator(module, region.id, &block.terminator, &block_index, &defined)?;
        }
    }

    let expected = ModuleFingerprint::of_module(module);
    if module.fingerprint != expected {
        return Err(diag("fingerprint_mismatch"));
    }
    Ok(())
}

fn verify_effect_edges(module: &ExecutionModule) -> Result<()> {
    let mut seen = HashSet::new();
    for edge in &module.effect_edges {
        if !seen.insert(edge.token) {
            return Err(diag("duplicate_effect_token"));
        }
    }
    for edge in &module.effect_edges {
        if let Some(prev) = edge.precedes_from {
            if prev == edge.token {
                return Err(diag("effect_self_predecessor"));
            }
            if !seen.contains(&prev) {
                return Err(diag("effect_predecessor_unknown"));
            }
        }
    }
    Ok(())
}

fn verify_terminator(
    module: &ExecutionModule,
    region: RegionId,
    terminator: &Terminator,
    block_index: &HashMap<(RegionId, BlockId), usize>,
    defined: &HashSet<SsaValueId>,
) -> Result<()> {
    let check_edge = |target: BlockId, arguments: &[SsaValueId]| -> Result<()> {
        let Some(idx) = block_index.get(&(region, target)).copied()
        else {
            return Err(diag("terminator_unknown_target"));
        };
        let block = &module.regions.iter().find(|r| r.id == region).expect("region").blocks[idx];
        if block.parameters.len() != arguments.len() {
            return Err(diag("terminator_arity_mismatch"));
        }
        for arg in arguments {
            if !defined.contains(arg) {
                return Err(diag("terminator_arg_undefined"));
            }
        }
        Ok(())
    };

    match terminator {
        Terminator::Branch { condition, then_edge, else_edge } => {
            if !defined.contains(condition) {
                return Err(diag("branch_condition_undefined"));
            }
            check_edge(then_edge.target, &then_edge.arguments)?;
            check_edge(else_edge.target, &else_edge.arguments)?;
        }
        Terminator::Switch { discriminant, cases, default } => {
            if !defined.contains(discriminant) {
                return Err(diag("switch_discriminant_undefined"));
            }
            for (_, edge) in cases {
                check_edge(edge.target, &edge.arguments)?;
            }
            check_edge(default.target, &default.arguments)?;
        }
        Terminator::Return { values } => {
            for value in values {
                if !defined.contains(value) {
                    return Err(diag("return_value_undefined"));
                }
            }
        }
        Terminator::Reject { .. } | Terminator::Unreachable => {}
        Terminator::Yield { values, resume } => {
            for value in values {
                if !defined.contains(value) {
                    return Err(diag("yield_value_undefined"));
                }
            }
            check_edge(resume.target, &resume.arguments)?;
        }
    }
    Ok(())
}

fn operands_of(kind: &OperationKind) -> Vec<SsaValueId> {
    match kind {
        OperationKind::LoadInput { .. } | OperationKind::LoadTerm { .. } | OperationKind::Constant { .. } => Vec::new(),
        OperationKind::ApplySemanticOperator { args, .. } | OperationKind::ApplyExtensionOperator { args, .. } => args.clone(),
        OperationKind::ConstructCollection { elements, .. } => elements.clone(),
        OperationKind::Index { target, .. } => vec![*target],
        OperationKind::ReadBinding { key } => vec![*key],
        OperationKind::WriteBinding { key, value, .. } => vec![*key, *value],
        OperationKind::RegisterRuleDispatch { head, pattern, replacement, .. } => vec![*head, *pattern, *replacement],
        OperationKind::RegisterCompiledRule { .. } => Vec::new(),
        OperationKind::EnterScope { parent } => parent.iter().copied().collect(),
        OperationKind::ExitScope { scope } => vec![*scope],
        OperationKind::CallProvider { args, .. } => args.clone(),
        OperationKind::Guard { predicate, .. } => vec![*predicate],
        OperationKind::MaterializeValue { source } | OperationKind::PublishResult { source } => vec![*source],
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "ExecutionIR.verifier").detail("reason", reason)
}
