//! `athena-vm` — typed `ExecutionIR` **执行运行时**（怎么跑）。
//!
//! `athena-engine` 是其**之上**的综合体（规划 / 编译 / 准入 / 域），不是并列解释器。
//!
//! 本 crate 只执行已编译模块：frame / slot / budget / cancellation / safepoint / 解释执行。
//! 热路径可用经审查的 `unsafe`（`unsafe_code = "deny"`，非 `forbid`）；不追求纯 safe 冒充高性能 VM。
//! **禁止**：前端字符串分派、Mathematica 名称解析、M-Graph admission、持久数学 payload owner、自建 GC。
//!
//! 依赖位置：
//! ```text
//! athena-types → athena-gc → … → athena-rewriter → athena-vm → athena-engine → athena
//! ```

#![deny(missing_docs)]

mod cancel;
mod config;
mod constant;
mod exit;
mod frame;
mod host;
mod instruction;
mod interpreter;
mod lease;
mod module;
mod slot;

pub use cancel::CancellationToken;
pub use config::VmConfig;
pub use constant::VmConstant;
pub use exit::VmExit;
pub use frame::{Frame, FrameStack};
pub use host::{HostOutcome, IndexAxesId, NullHost, ProviderOpId, SemanticOpId, VmHost};
pub use instruction::{ConstantIndex, Instruction, MAX_HOST_ARGS, SlotIndex};
pub use interpreter::{Interpreter, VmExecutor};
pub use lease::ExecutionLease;
pub use module::{ModuleFingerprint, VmModule};
pub use slot::{SlotTable, SlotValue};
