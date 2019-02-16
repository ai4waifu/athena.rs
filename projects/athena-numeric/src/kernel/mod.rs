//! Machine kernel：无所有权、无分配、无 GC，只写 Athena limb view。
//!
//! ```text
//! kernel/
//!   buffer.rs      — LimbBuffer / ScratchWorkspace
//!   limb.rs        — 默认再导出（value/arithmetic 入口）
//!   portable/      — ISA-agnostic 算法实现（语义基线）
//!   table.rs       — KernelTable 绑定
//!   x86_64 / …     — ISA KernelTable
//! ```
//!
//! 算法策略见 [`crate::algorithm`]。宿主合同见 [`crate::dispatch::PortableBackend`]。
//! 外部 object API 见 [`crate::foreign`]。

pub(crate) mod buffer;
pub(crate) mod limb;
pub(crate) mod portable;
pub(crate) mod table;
pub(crate) mod token;

#[cfg(all(target_arch = "x86_64", not(target_family = "wasm")))]
pub(crate) mod x86_64;

pub use buffer::ScratchWorkspace;
pub(crate) use buffer::{LimbBuffer, kernel_err};
pub use table::KernelTable;
pub use token::{ExecutionToken, KernelPreconditions};
