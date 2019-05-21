//! KernelIR 合同：执行单元指令与已验证计划摘要。

pub mod artifact;
pub mod unit;

pub use artifact::{KernelIR, KernelOperation};
pub use unit::{ExecUnit, HandlerId, Instr};
