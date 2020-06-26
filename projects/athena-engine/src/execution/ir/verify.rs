//! [`ExecutionModule`](super::ExecutionModule) 的结构校验器。
//!
//! 覆盖定义唯一性、同块 SSA 顺序、控制边、effect-token
//! 配对 / 链成员关系、guard 出口表、入口可达性、支配关系，以及指纹重算。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::{
    ModuleFingerprint,
    ids::{BlockId, RegionId, SsaValueId},
    module::ExecutionModule,
    operation::{GuardFailure, OperationKind},
    terminator::Terminator,
};

use std::collections::{HashMap, HashSet, VecDeque};

/// 校验 module 在结构上可被后端接受。
pub fn verify_module(module: &ExecutionModule) -> Result<()> {
    if module.regions.is_empty() {
        return Err(diag("empty_regions"));
    }
    verify_effect_edges(module)?;

    let mut defined = HashSet::new();
    let mut def_block: HashMap<SsaValueId, (RegionId, BlockId)> = HashMap::new();
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

        let cfg = RegionCfg::build(region)?;
        // 迁移期允许不可达块（如 `Recover` handler 在无 `Reject` 时冷路径）。
        // 支配关系与跨块 SSA 使用仅在可达子图上强制。

        // 先登记全部定义，避免依赖 `blocks` 向量顺序误判跨块 use。
        for block in &region.blocks {
            for param in &block.parameters {
                if !defined.insert(param.value) {
                    return Err(diag("duplicate_ssa_definition"));
                }
                def_block.insert(param.value, (region.id, block.id));
            }
            for op in &block.operations {
                if let Some(result) = op.result {
                    if !defined.insert(result) {
                        return Err(diag("duplicate_ssa_definition"));
                    }
                    def_block.insert(result, (region.id, block.id));
                }
            }
        }

        for block in &region.blocks {
            let block_reachable = cfg.reachable.contains(&block.id);
            let mut local_defs = HashSet::new();
            for param in &block.parameters {
                local_defs.insert(param.value);
            }
            for op in &block.operations {
                for used in operands_of(&op.kind) {
                    if block_reachable {
                        verify_ssa_use(used, region.id, block.id, &local_defs, &def_block, &cfg)?;
                    }
                    else {
                        verify_ssa_use_local_only(used, region.id, block.id, &local_defs, &def_block)?;
                    }
                }
                if let Some(result) = op.result {
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
            if block_reachable {
                verify_terminator(module, region.id, block.id, &block.terminator, &block_index, &local_defs, &def_block, &cfg)?;
            }
            else {
                // 不可达块仍校验边目标存在与 arity，但不做支配证明。
                verify_terminator_shape(module, region.id, &block.terminator, &block_index)?;
            }
        }
    }

    let expected = ModuleFingerprint::of_module(module);
    if module.fingerprint != expected {
        return Err(diag("fingerprint_mismatch"));
    }
    Ok(())
}

fn verify_ssa_use_local_only(
    used: SsaValueId,
    region: RegionId,
    use_block: BlockId,
    local_defs: &HashSet<SsaValueId>,
    def_block: &HashMap<SsaValueId, (RegionId, BlockId)>,
) -> Result<()> {
    if local_defs.contains(&used) {
        return Ok(());
    }
    match def_block.get(&used) {
        None => Err(diag("use_before_def")),
        Some(&(def_region, def_bid)) if def_region == region && def_bid == use_block => Err(diag("use_before_def")),
        Some(_) => Ok(()), // 不可达块上的跨块 use：Recover 冷路径迁移容忍
    }
}

fn verify_ssa_use(
    used: SsaValueId,
    region: RegionId,
    use_block: BlockId,
    local_defs: &HashSet<SsaValueId>,
    def_block: &HashMap<SsaValueId, (RegionId, BlockId)>,
    cfg: &RegionCfg,
) -> Result<()> {
    // 同块：必须已出现在本块参数或此前操作的结果中（禁止同块 use-before-def）。
    if local_defs.contains(&used) {
        return Ok(());
    }
    let Some(&(def_region, def_bid)) = def_block.get(&used)
    else {
        return Err(diag("use_before_def"));
    };
    if def_region != region {
        return Err(diag("cross_region_ssa_use"));
    }
    // 同块但尚未进入 local_defs ⇒ 同块顺序错误。
    if def_bid == use_block {
        return Err(diag("use_before_def"));
    }
    if !cfg.dominates(def_bid, use_block) {
        return Err(diag("ssa_def_does_not_dominate_use"));
    }
    Ok(())
}

struct RegionCfg {
    entry: BlockId,
    /// block id → dense index
    index_of: HashMap<BlockId, usize>,
    /// immediate dominator by dense index (`idom[entry] == entry`)
    idom: Vec<usize>,
    reachable: HashSet<BlockId>,
}

impl RegionCfg {
    fn build(region: &super::region::Region) -> Result<Self> {
        let n = region.blocks.len();
        let mut index_of = HashMap::with_capacity(n);
        let mut id_of = Vec::with_capacity(n);
        for (idx, block) in region.blocks.iter().enumerate() {
            index_of.insert(block.id, idx);
            id_of.push(block.id);
        }
        let entry_idx = *index_of.get(&region.entry).ok_or_else(|| diag("missing_entry_block"))?;

        let mut succs = vec![Vec::new(); n];
        for block in &region.blocks {
            let from = index_of[&block.id];
            for target in terminator_targets(&block.terminator) {
                let Some(&to) = index_of.get(&target)
                else {
                    return Err(diag("terminator_unknown_target"));
                };
                succs[from].push(to);
            }
        }

        let mut reachable_idx = HashSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(entry_idx);
        reachable_idx.insert(entry_idx);
        while let Some(u) = queue.pop_front() {
            for &v in &succs[u] {
                if reachable_idx.insert(v) {
                    queue.push_back(v);
                }
            }
        }

        let mut reachable = HashSet::new();
        for &idx in &reachable_idx {
            reachable.insert(id_of[idx]);
        }

        let mut preds = vec![Vec::new(); n];
        for u in 0..n {
            if !reachable_idx.contains(&u) {
                continue;
            }
            for &v in &succs[u] {
                if reachable_idx.contains(&v) {
                    preds[v].push(u);
                }
            }
        }

        let rpo = reverse_postorder(entry_idx, &succs, &reachable_idx);
        let mut rpo_number = vec![0usize; n];
        for (num, &b) in rpo.iter().enumerate() {
            rpo_number[b] = num;
        }

        let mut idom = vec![usize::MAX; n];
        idom[entry_idx] = entry_idx;
        let mut changed = true;
        while changed {
            changed = false;
            for &b in &rpo {
                if b == entry_idx {
                    continue;
                }
                let mut new_idom = None;
                for &p in &preds[b] {
                    if idom[p] == usize::MAX {
                        continue;
                    }
                    new_idom = Some(match new_idom {
                        None => p,
                        Some(d) => intersect(d, p, &idom, &rpo_number),
                    });
                }
                if let Some(d) = new_idom {
                    if idom[b] != d {
                        idom[b] = d;
                        changed = true;
                    }
                }
            }
        }

        for &idx in &reachable_idx {
            if idom[idx] == usize::MAX {
                return Err(diag("idom_incomplete"));
            }
        }

        Ok(Self { entry: region.entry, index_of, idom, reachable })
    }

    fn dominates(&self, def: BlockId, use_block: BlockId) -> bool {
        if def == use_block {
            return true;
        }
        let Some(&def_i) = self.index_of.get(&def)
        else {
            return false;
        };
        let Some(&use_i) = self.index_of.get(&use_block)
        else {
            return false;
        };
        if !self.reachable.contains(&def) || !self.reachable.contains(&use_block) {
            return false;
        }
        let entry_i = self.index_of[&self.entry];
        let mut cur = use_i;
        while cur != def_i {
            if cur == entry_i {
                return false;
            }
            let parent = self.idom[cur];
            if parent == cur {
                return def_i == cur;
            }
            cur = parent;
        }
        true
    }
}

fn reverse_postorder(entry: usize, succs: &[Vec<usize>], reachable: &HashSet<usize>) -> Vec<usize> {
    let n = succs.len();
    let mut visited = vec![false; n];
    let mut post = Vec::new();
    fn dfs(u: usize, succs: &[Vec<usize>], reachable: &HashSet<usize>, visited: &mut [bool], post: &mut Vec<usize>) {
        visited[u] = true;
        for &v in &succs[u] {
            if reachable.contains(&v) && !visited[v] {
                dfs(v, succs, reachable, visited, post);
            }
        }
        post.push(u);
    }
    if reachable.contains(&entry) {
        dfs(entry, succs, reachable, &mut visited, &mut post);
    }
    post.reverse();
    post
}

fn intersect(mut finger1: usize, mut finger2: usize, idom: &[usize], rpo_number: &[usize]) -> usize {
    while finger1 != finger2 {
        while rpo_number[finger1] > rpo_number[finger2] {
            finger1 = idom[finger1];
        }
        while rpo_number[finger2] > rpo_number[finger1] {
            finger2 = idom[finger2];
        }
    }
    finger1
}

fn terminator_targets(terminator: &Terminator) -> Vec<BlockId> {
    match terminator {
        Terminator::Branch { then_edge, else_edge, .. } => {
            vec![then_edge.target, else_edge.target]
        }
        Terminator::Switch { cases, default, .. } => {
            let mut targets: Vec<_> = cases.iter().map(|(_, e)| e.target).collect();
            targets.push(default.target);
            targets
        }
        Terminator::Yield { resume, .. } => vec![resume.target],
        Terminator::Return { .. } | Terminator::Reject { .. } | Terminator::Unreachable => Vec::new(),
    }
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

fn verify_terminator_shape(
    module: &ExecutionModule,
    region: RegionId,
    terminator: &Terminator,
    block_index: &HashMap<(RegionId, BlockId), usize>,
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
        Ok(())
    };
    match terminator {
        Terminator::Branch { then_edge, else_edge, .. } => {
            check_edge(then_edge.target, &then_edge.arguments)?;
            check_edge(else_edge.target, &else_edge.arguments)?;
        }
        Terminator::Switch { cases, default, .. } => {
            for (_, edge) in cases {
                check_edge(edge.target, &edge.arguments)?;
            }
            check_edge(default.target, &default.arguments)?;
        }
        Terminator::Yield { resume, .. } => {
            check_edge(resume.target, &resume.arguments)?;
        }
        Terminator::Return { .. } | Terminator::Reject { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}

fn verify_terminator(
    module: &ExecutionModule,
    region: RegionId,
    block_id: BlockId,
    terminator: &Terminator,
    block_index: &HashMap<(RegionId, BlockId), usize>,
    local_defs: &HashSet<SsaValueId>,
    def_block: &HashMap<SsaValueId, (RegionId, BlockId)>,
    cfg: &RegionCfg,
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
            verify_ssa_use(*arg, region, block_id, local_defs, def_block, cfg)?;
        }
        Ok(())
    };

    match terminator {
        Terminator::Branch { condition, then_edge, else_edge } => {
            verify_ssa_use(*condition, region, block_id, local_defs, def_block, cfg)?;
            check_edge(then_edge.target, &then_edge.arguments)?;
            check_edge(else_edge.target, &else_edge.arguments)?;
        }
        Terminator::Switch { discriminant, cases, default } => {
            verify_ssa_use(*discriminant, region, block_id, local_defs, def_block, cfg)?;
            for (_, edge) in cases {
                check_edge(edge.target, &edge.arguments)?;
            }
            check_edge(default.target, &default.arguments)?;
        }
        Terminator::Return { values } => {
            for value in values {
                verify_ssa_use(*value, region, block_id, local_defs, def_block, cfg)?;
            }
        }
        Terminator::Reject { .. } | Terminator::Unreachable => {}
        Terminator::Yield { values, resume } => {
            for value in values {
                verify_ssa_use(*value, region, block_id, local_defs, def_block, cfg)?;
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
        OperationKind::MaterializeValue { source } | OperationKind::PublishResult { source } => {
            vec![*source]
        }
    }
}

fn diag(reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("component", "ExecutionIR.verifier").detail("reason", reason)
}
