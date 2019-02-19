//! Machine kernel：无所有权、无分配、无 GC，只写 Athena limb view。
//!
//! ```text
//! kernel/
//!   buffer.rs      — LimbBuffer / ScratchWorkspace（调用期缓冲合同）
//!   limb.rs        — 默认 KernelTable 再导出（value/arithmetic 入口）
//!   portable/      — ISA-agnostic 算法实现（语义基线）
//!   pure_rust/     — 宿主合同门面（`PureRustBackend`；Living 17 步骤 4 改名）
//!   （后续）x86_64 / aarch64 / wasm — ISA KernelTable
//! ```
//!
//! 算法策略见 [`crate::algorithm`]。宿主能力门面见 [`crate::dispatch`]。
//! 外部 object API 见 [`crate::foreign`]。

pub(crate) mod buffer;
pub(crate) mod limb;
pub(crate) mod portable;
pub(crate) mod pure_rust;
pub(crate) mod table;
pub(crate) mod token;

#[cfg(all(target_arch = "x86_64", not(target_family = "wasm")))]
pub(crate) mod x86_64;

pub use buffer::ScratchWorkspace;
pub(crate) use buffer::{LimbBuffer, kernel_err};
pub(crate) use pure_rust::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES;
pub use pure_rust::PureRustBackend;
pub use table::KernelTable;
pub use token::{ExecutionToken, KernelPreconditions};
