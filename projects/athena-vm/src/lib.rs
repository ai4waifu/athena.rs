//! `athena-vm` — 受限的 typed `ExecutionIR` 运行时。
//!
//! 本 crate 只执行已编译模块：frame / slot / budget / cancellation / safepoint / 解释执行。
//! **禁止**：前端字符串分派、Mathematica 名称解析、M-Graph admission、持久数学 payload owner、自建 GC。
//!
//! 依赖位置：
//! ```text
//! athena-types → athena-gc → … → athena-rewriter → athena-vm → athena-engine → athena
//! ```

#![deny(missing_docs)]

mod config;
mod exit;
mod instruction;
mod interpreter;
mod module;

pub use config::VmConfig;
pub use exit::VmExit;
pub use instruction::Instruction;
pub use interpreter::{Interpreter, VmExecutor};
pub use module::{ModuleFingerprint, VmModule};
