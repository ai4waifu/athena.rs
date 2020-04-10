//! 封闭指令集（骨架）。
//!
//! 指令**不得**携带 `&str` / 方言表面名。后续接入 `ApplySemanticOperator` /
//! `CallProvider` 时只接受封闭 typed ID。

/// 槽下标（相对当前帧基址的局部下标，或绝对槽下标，由指令约定）。
pub type SlotIndex = u32;

/// 常量表下标。
pub type ConstantIndex = u32;

/// VM 指令（最小闭集 · 无语义算子）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    /// 正常返回。
    Return,
    /// GC / 取消检查点。
    Safepoint,
    /// 将常量表项写入绝对槽。
    LoadConstant {
        /// 目标槽。
        dst: SlotIndex,
        /// 常量表下标。
        constant: ConstantIndex,
    },
    /// 绝对槽之间复制。
    Move {
        /// 目标槽。
        dst: SlotIndex,
        /// 源槽。
        src: SlotIndex,
    },
    /// 谓词槽为 false 时走 [`crate::exit::VmExit::Rejected`]。
    Guard {
        /// Boolean 谓词槽。
        predicate: SlotIndex,
    },
    /// 显式拒绝。
    Reject,
}
