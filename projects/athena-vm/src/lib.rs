//! `athena-vm` — 受限的 typed `ExecutionIR` 运行时。
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
mod exit;
mod frame;
mod instruction;
mod interpreter;
mod module;
mod slot;

pub use cancel::CancellationToken;
pub use config::VmConfig;
pub use exit::VmExit;
pub use frame::{Frame, FrameStack};
pub use instruction::Instruction;
pub use interpreter::{Interpreter, VmExecutor};
pub use module::{ModuleFingerprint, VmModule};
pub use slot::{SlotTable, SlotValue};
