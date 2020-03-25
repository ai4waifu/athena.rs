//! `ExecutionIR` 的会话局部标识（不是 `TermId` / `ValueId` / `ResultId`）。

/// 单个 [`super::ExecutionModule`] 内的 SSA 值标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SsaValueId(pub u32);

/// 单个 region 内的基本块标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

/// 单个 module 内的 region 标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RegionId(pub u32);

/// 连接有副作用操作的有序 effect token。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EffectToken(pub u32);

/// module 级常量表下标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConstantId(pub u32);

/// module 级输入表下标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InputId(pub u32);

/// module 级捕获根表下标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapturedRootId(pub u32);

/// module 级 provider 调用描述符表下标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProviderCallId(pub u32);

/// module 级 guard / failure / deoptimization 出口表下标。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExitId(pub u32);
