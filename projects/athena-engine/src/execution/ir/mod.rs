//! Typed `ExecutionIR` — region-based SSA for reference / JIT / WASM backends.
//!
//! This module freezes the executable IR contract. It is **not** a rename of
//! `KernelIR` / `ExecUnit` / stack VM instructions. Dialect surface names must
//! not appear in opcodes or descriptors.

pub mod block;
pub mod effect;
pub mod exit;
pub mod fingerprint;
pub mod ids;
pub mod module;
pub mod operation;
pub mod region;
pub mod terminator;
pub mod types;
pub mod verify;

pub use block::{BasicBlock, BlockParameter};
pub use effect::{EffectEdge, EffectKind};
pub use exit::{DeclaredExit, ExitKind};
pub use fingerprint::ModuleFingerprint;
pub use ids::{
    BlockId, CapturedRootId, ConstantId, EffectToken, ExitId, InputId, ProviderCallId, RegionId, SsaValueId,
};
pub use module::{ExecutionIR, ExecutionModule};
pub use operation::{GuardFailure, Operation, OperationKind};
pub use region::Region;
pub use terminator::{BlockEdge, Terminator};
pub use types::{CapturedRoot, ConstantValue, ExecutionValueType, ModuleInput, ProviderCallDescriptor};
pub use verify::verify_module;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_module_has_stable_shape() {
        let module = ExecutionModule::empty();
        assert_eq!(module.regions.len(), 1);
        assert_eq!(module.entry_region(), Some(RegionId(0)));
        assert_eq!(module.fingerprint, ModuleFingerprint::of_module(&module));
    }
}
