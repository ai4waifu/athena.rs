//! Session-local identities for `ExecutionIR` (not `TermId` / `ValueId` / `ResultId`).

/// SSA value identity inside one [`super::ExecutionModule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SsaValueId(pub u32);

/// Basic block identity inside one region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// Region identity inside one module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(pub u32);

/// Ordered effect token linking effectful operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectToken(pub u32);

/// Index into module-level constant table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantId(pub u32);

/// Index into module-level input table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputId(pub u32);

/// Index into module-level captured-root table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapturedRootId(pub u32);

/// Index into module-level provider-call descriptor table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderCallId(pub u32);

/// Index into module-level guard / failure / deoptimization exit tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExitId(pub u32);
