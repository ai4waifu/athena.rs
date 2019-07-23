//! Structural verifiers for [`ExecutionModule`](super::ExecutionModule).
//!
//! Bootstrap covers definition uniqueness, same-block SSA order, control edges,
//! and fingerprint recomputation. Full dominance / effect-order proofs follow in
//! later cutover commits.

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::{
    ModuleFingerprint,
    ids::{BlockId, RegionId, SsaValueId},
    module::ExecutionModule,
    operation::OperationKind,
    terminator::Terminator,
};

use std::collections::{HashMap, HashSet};

/// Verify a module is structurally admissible for backends.
pub fn verify_module(module: &ExecutionModule) -> Result<()> {
    if module.regions.is_empty() {
        return Err(diag("empty_regions"));
    }
    let mut defined = HashSet::new();
    let mut block_index: HashMap<(RegionId, BlockId), usize> = HashMap::new();

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
                        // Cross-block uses require block-arg wiring; bootstrap only
                        // allows values already defined earlier in the module walk
                        // (predecessors must pass them as block arguments later).
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
                if op.effect_in.is_some() != op.effect_out.is_some() {
                    return Err(diag("effect_token_pair_mismatch"));
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
        OperationKind::ApplySemanticOperator { args, .. } => args.clone(),
        OperationKind::MakeList { elements } => elements.clone(),
        OperationKind::ReadBinding { key } => vec![*key],
        OperationKind::WriteBinding { key, value, .. } => vec![*key, *value],
        OperationKind::WriteDownValue { key, pattern, value } => vec![*key, *pattern, *value],
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::ir::{
        BasicBlock, BlockEdge, BlockId, ConstantId, ConstantValue, ExecutionModule, ExecutionValueType, Operation, Region, RegionId,
        SsaValueId, Terminator,
    };

    #[test]
    fn empty_module_verifies() {
        let module = ExecutionModule::empty();
        verify_module(&module).expect("empty module");
    }

    #[test]
    fn constant_return_verifies() {
        let v0 = SsaValueId(0);
        let block = BasicBlock {
            id: BlockId(0),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(v0),
                result_type: ExecutionValueType::Boolean,
                kind: crate::execution::ir::OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::return_value(v0),
        };
        let region = Region::from_entry_block(RegionId(0), block, vec![ExecutionValueType::Boolean]);
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: vec![ConstantValue::boolean(true)],
            captured_roots: Vec::new(),
            regions: vec![region],
            effect_edges: Vec::new(),
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        verify_module(&module).expect("constant return");
    }

    #[test]
    fn use_before_def_rejected() {
        let block =
            BasicBlock { id: BlockId(0), parameters: Vec::new(), operations: Vec::new(), terminator: Terminator::return_value(SsaValueId(99)) };
        let region = Region::from_entry_block(RegionId(0), block, Vec::new());
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: Vec::new(),
            captured_roots: Vec::new(),
            regions: vec![region],
            effect_edges: Vec::new(),
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        assert!(verify_module(&module).is_err());
    }

    #[test]
    fn branch_targets_must_exist() {
        let cond = SsaValueId(0);
        let entry = BasicBlock {
            id: BlockId(0),
            parameters: Vec::new(),
            operations: vec![Operation {
                result: Some(cond),
                result_type: ExecutionValueType::Boolean,
                kind: crate::execution::ir::OperationKind::Constant { constant: ConstantId(0) },
                effect_in: None,
                effect_out: None,
            }],
            terminator: Terminator::Branch { condition: cond, then_edge: BlockEdge::jump(BlockId(1)), else_edge: BlockEdge::jump(BlockId(2)) },
        };
        let region = Region { id: RegionId(0), entry: BlockId(0), blocks: vec![entry], result_types: Vec::new() };
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: vec![ConstantValue::boolean(true)],
            captured_roots: Vec::new(),
            regions: vec![region],
            effect_edges: Vec::new(),
            exits: Vec::new(),
            provider_calls: Vec::new(),
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        assert!(verify_module(&module).is_err());
    }
}
