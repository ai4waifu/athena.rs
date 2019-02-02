//! Machine kernel：无所有权、无分配、无 GC，只写 Athena limb view。
//!
//! ```text
//! kernel/
//!   buffer.rs      — LimbBuffer / ScratchWorkspace（调用期缓冲合同）
//!   limb.rs        — 默认 KernelTable 再导出（value/arithmetic 入口）
//!   pure_rust/     — 语义基线机器内核
//!   （后续）x86_64 / aarch64 / wasm — ISA KernelTable
//! ```
//!
//! 算法策略见 [`crate::algorithm`]。宿主能力门面见 [`crate::dispatch`]。
//! 外部 object API 见 [`crate::foreign`]。

pub(crate) mod buffer;
pub(crate) mod limb;
pub(crate) mod pure_rust;
pub(crate) mod table;

#[cfg(all(target_arch = "x86_64", not(target_family = "wasm")))]
pub(crate) mod x86_64;

pub(crate) use buffer::{LimbBuffer, kernel_err};
pub use buffer::ScratchWorkspace;
pub(crate) use pure_rust::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES;
pub use pure_rust::PureRustBackend;
pub use table::KernelTable;
