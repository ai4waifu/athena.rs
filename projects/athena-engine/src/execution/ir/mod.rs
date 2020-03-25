//! 类型化的 `ExecutionIR` — 面向 reference / JIT / WASM 后端的基于 region 的 SSA。
//!
//! 本模块冻结可执行 IR 合同。它**不是**栈式字节码或操作数栈 VM。
//! 方言表层名称不得出现在操作码或描述符中。

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
pub use ids::{BlockId, CapturedRootId, ConstantId, EffectToken, ExitId, InputId, ProviderCallId, RegionId, SsaValueId};
pub use module::{ExecutionIR, ExecutionModule};
pub use operation::{GuardFailure, Operation, OperationKind};
pub use region::Region;
pub use terminator::{BlockEdge, Terminator};
pub use types::{CapturedRoot, ConstantValue, ExecutionValueType, ModuleInput, ProviderCallDescriptor};
pub use verify::verify_module;
