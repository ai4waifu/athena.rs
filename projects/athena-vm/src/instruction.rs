//! 封闭指令集（骨架）。
//!
//! 指令**不得**携带 `&str` / 方言表面名。语义 / provider 只带 opaque typed ID，
//! 经 [`crate::host::VmHost`] 回调由 engine 实现。

use crate::host::{ProviderOpId, SemanticOpId};

/// 槽下标（绝对槽下标，由指令约定）。
pub type SlotIndex = u32;

/// 常量表下标。
pub type ConstantIndex = u32;

/// 每条 host 调用边最多携带的实参槽数（骨架上限）。
pub const MAX_HOST_ARGS: usize = 4;

/// VM 指令（最小闭集）。
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
    /// 经 [`crate::host::VmHost::apply_semantic`] 应用封闭语义算子。
    ApplySemantic {
        /// 结果槽。
        dst: SlotIndex,
        /// opaque 语义算子。
        op: SemanticOpId,
        /// 有效实参个数（≤ [`MAX_HOST_ARGS`]）。
        argc: u8,
        /// 实参槽（仅前 `argc` 个有效）。
        args: [SlotIndex; MAX_HOST_ARGS],
    },
    /// 经 [`crate::host::VmHost::call_provider`] 调用类型化 provider。
    CallProvider {
        /// 结果槽。
        dst: SlotIndex,
        /// opaque provider 调用点。
        op: ProviderOpId,
        /// 有效实参个数（≤ [`MAX_HOST_ARGS`]）。
        argc: u8,
        /// 实参槽（仅前 `argc` 个有效）。
        args: [SlotIndex; MAX_HOST_ARGS],
    },
}

impl Instruction {
    /// 构造一元 `ApplySemantic`。
    pub const fn apply_semantic1(dst: SlotIndex, op: SemanticOpId, arg0: SlotIndex) -> Self {
        Self::ApplySemantic {
            dst,
            op,
            argc: 1,
            args: [arg0, 0, 0, 0],
        }
    }

    /// 构造二元 `ApplySemantic`。
    pub const fn apply_semantic2(dst: SlotIndex, op: SemanticOpId, arg0: SlotIndex, arg1: SlotIndex) -> Self {
        Self::ApplySemantic {
            dst,
            op,
            argc: 2,
            args: [arg0, arg1, 0, 0],
        }
    }

    /// 构造一元 `CallProvider`。
    pub const fn call_provider1(dst: SlotIndex, op: ProviderOpId, arg0: SlotIndex) -> Self {
        Self::CallProvider {
            dst,
            op,
            argc: 1,
            args: [arg0, 0, 0, 0],
        }
    }
}
