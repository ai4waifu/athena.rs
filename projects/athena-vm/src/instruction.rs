//! 封闭指令集（骨架）。
//!
//! 指令**不得**携带 `&str` / 方言表面名。后续接入 `ApplySemanticOperator` /
//! `CallProvider` 时只接受封闭 typed ID。

/// VM 指令（最小闭集）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    /// 正常返回。
    Return,
    /// GC / 取消检查点（骨架为空操作，仅计入步数）。
    Safepoint,
}
