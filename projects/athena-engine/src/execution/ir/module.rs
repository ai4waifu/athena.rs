//! Top-level `ExecutionModule` — the only executable IR unit.

use super::effect::EffectEdge;
use super::exit::DeclaredExit;
use super::fingerprint::ModuleFingerprint;
use super::ids::RegionId;
use super::region::Region;
use super::types::{CapturedRoot, ConstantValue, ModuleInput, ProviderCallDescriptor};

/// Verified executable module produced by [`crate::execution::compiler::ExecutionCompiler`].
///
/// This is not an AST, bytecode stream, or task queue. Backends consume the same
/// module: reference executor, native JIT, WASM, and domain kernels via
/// `CallProvider`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionModule {
    /// Request / snapshot inputs.
    pub inputs: Vec<ModuleInput>,
    /// Compile-time constants.
    pub constants: Vec<ConstantValue>,
    /// Captured runtime roots (IR does not own them).
    pub captured_roots: Vec<CapturedRoot>,
    /// Control-flow regions.
    pub regions: Vec<Region>,
    /// Ordered effect signatures.
    pub effect_edges: Vec<EffectEdge>,
    /// Guard / failure / deoptimization exits.
    pub exits: Vec<DeclaredExit>,
    /// Typed provider call descriptors.
    pub provider_calls: Vec<ProviderCallDescriptor>,
    /// Source-independent structural fingerprint.
    pub fingerprint: ModuleFingerprint,
}

impl ExecutionModule {
    /// Empty module with a single empty entry region (contract freeze placeholder).
    pub fn empty() -> Self {
        use super::block::BasicBlock;
        use super::ids::BlockId;

        let entry = BasicBlock::empty_return(BlockId(0));
        let region = Region::from_entry_block(RegionId(0), entry, Vec::new());
        let mut module = Self {
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
        module
    }

    /// Entry region id when present.
    pub fn entry_region(&self) -> Option<RegionId> {
        self.regions.first().map(|r| r.id)
    }
}

/// Public alias used in design prose (`ExecutionIR` ≡ verified [`ExecutionModule`] graph).
pub type ExecutionIR = ExecutionModule;
