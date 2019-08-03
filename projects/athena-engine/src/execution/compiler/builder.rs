//! Module construction helpers for [super::ExecutionCompiler].

use athena_types::{Result, TermId};

use crate::execution::ir::{
    BasicBlock, BlockId, CapturedRoot, CapturedRootId, ConstantId, ConstantValue, EffectEdge, EffectKind, EffectToken,
    ExecutionModule, ExecutionValueType, ModuleFingerprint, ProviderCallDescriptor, ProviderCallId, Region, RegionId, SsaValueId,
    verify_module,
};

#[derive(Default)]
pub(super) struct ModuleBuilder {
    constants: Vec<ConstantValue>,
    captured_roots: Vec<CapturedRoot>,
    effect_edges: Vec<EffectEdge>,
    provider_calls: Vec<ProviderCallDescriptor>,
    next_ssa: u32,
    next_block: u32,
    next_effect: u32,
}

impl ModuleBuilder {
    pub(super) fn ssa(&mut self) -> SsaValueId {
        let id = SsaValueId(self.next_ssa);
        self.next_ssa = self.next_ssa.saturating_add(1);
        id
    }

    pub(super) fn block_id(&mut self) -> BlockId {
        let id = BlockId(self.next_block);
        self.next_block = self.next_block.saturating_add(1);
        id
    }

    pub(super) fn push_constant(&mut self, value: ConstantValue) -> ConstantId {
        let id = ConstantId(self.constants.len() as u32);
        self.constants.push(value);
        id
    }

    pub(super) fn push_term_root(&mut self, term: TermId) -> CapturedRootId {
        let id = CapturedRootId(self.captured_roots.len() as u32);
        self.captured_roots.push(CapturedRoot::term(term));
        id
    }

    pub(super) fn push_effect(&mut self, kind: EffectKind, precedes_from: Option<EffectToken>) -> EffectToken {
        let token = EffectToken(self.next_effect);
        self.next_effect = self.next_effect.saturating_add(1);
        self.effect_edges.push(match precedes_from {
            Some(prev) => EffectEdge::after(token, prev, kind),
            None => EffectEdge::entry(token, kind),
        });
        token
    }

    pub(super) fn push_provider_call(&mut self, descriptor: ProviderCallDescriptor) -> ProviderCallId {
        let id = ProviderCallId(self.provider_calls.len() as u32);
        let mut descriptor = descriptor;
        descriptor.id = id;
        self.provider_calls.push(descriptor);
        id
    }

    pub(super) fn finish(self, blocks: Vec<BasicBlock>, entry: BlockId) -> Result<ExecutionModule> {
        let region = Region { id: RegionId(0), entry, blocks, result_types: vec![ExecutionValueType::Term] };
        let mut module = ExecutionModule {
            inputs: Vec::new(),
            constants: self.constants,
            captured_roots: self.captured_roots,
            regions: vec![region],
            effect_edges: self.effect_edges,
            exits: Vec::new(),
            provider_calls: self.provider_calls,
            fingerprint: ModuleFingerprint(0),
        };
        module.fingerprint = ModuleFingerprint::of_module(&module);
        verify_module(&module)?;
        Ok(module)
    }
}

