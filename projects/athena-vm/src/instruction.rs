//! 封闭指令集（骨架）。
//!
//! 指令**不得**携带 `&str` / 方言表面名。语义 / provider 只带 opaque typed ID，
//! 经 [`crate::host::VmHost`] 回调由 engine 实现。

use athena_types::{BindingEvaluationPolicy, BindingKind, CollectionKind};

use crate::host::{ExtensionOpId, IndexAxesId, ProviderOpId, SemanticOpId};

/// 槽下标（绝对槽下标，由指令约定）。
pub type SlotIndex = u32;

/// 常量表下标。
pub type ConstantIndex = u32;

/// 每条 host 调用边最多携带的实参槽数（含集合构造）。
pub const MAX_HOST_ARGS: usize = 16;

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
    /// 无条件跳转到指令下标。
    Jump {
        /// 目标 PC（指令下标）。
        target: u32,
    },
    /// Boolean 条件分支。
    Branch {
        /// 谓词槽。
        condition: SlotIndex,
        /// 真分支 PC。
        then_pc: u32,
        /// 假分支 PC。
        else_pc: u32,
    },
    /// 正常返回并记录结果槽。
    ReturnValue {
        /// 结果绝对槽。
        slot: SlotIndex,
    },
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
    /// 经 [`crate::host::VmHost::read_binding`] 读取 Session / 作用域绑定。
    ReadBinding {
        /// 结果槽。
        dst: SlotIndex,
        /// 绑定键槽（须为 [`crate::slot::SlotValue::Symbol`]）。
        key: SlotIndex,
    },
    /// 经 [`crate::host::VmHost::write_binding`] 写入 Session / 作用域绑定。
    WriteBinding {
        /// 结果槽（通常为 Unit）。
        dst: SlotIndex,
        /// 绑定键槽。
        key: SlotIndex,
        /// 写入值槽。
        value: SlotIndex,
        /// 绑定类别。
        kind: BindingKind,
        /// 求值策略。
        evaluation: BindingEvaluationPolicy,
    },
    /// 经 [`crate::host::VmHost::enter_scope`] 进入作用域帧。
    EnterScope {
        /// 结果槽（[`crate::slot::SlotValue::Scope`] 深度句柄）。
        dst: SlotIndex,
        /// 可选父作用域槽；`None` 表示压入当前顶帧之上。
        parent: Option<SlotIndex>,
    },
    /// 经 [`crate::host::VmHost::exit_scope`] 退出作用域帧（无结果槽）。
    ExitScope {
        /// 由 [`Self::EnterScope`] 写入的作用域句柄槽。
        scope: SlotIndex,
    },
    /// 经 [`crate::host::VmHost::construct_collection`] 构造类型化集合。
    ConstructCollection {
        /// 结果槽。
        dst: SlotIndex,
        /// 集合种类。
        kind: CollectionKind,
        /// 有效元素个数（≤ [`MAX_HOST_ARGS`]）。
        argc: u8,
        /// 元素槽（仅前 `argc` 个有效）。
        args: [SlotIndex; MAX_HOST_ARGS],
    },
    /// 经 [`crate::host::VmHost::apply_index`] 对目标做下标访问。
    Index {
        /// 结果槽。
        dst: SlotIndex,
        /// 目标槽。
        target: SlotIndex,
        /// 轴规格表 ID（host 侧 `IndexSpec` 序列）。
        axes: IndexAxesId,
    },
    /// 经 [`crate::host::VmHost::apply_extension`] 应用扩展算子。
    ApplyExtension {
        /// 结果槽。
        dst: SlotIndex,
        /// opaque 扩展算子。
        op: ExtensionOpId,
        /// 有效实参个数（≤ [`MAX_HOST_ARGS`]）。
        argc: u8,
        /// 实参槽（仅前 `argc` 个有效）。
        args: [SlotIndex; MAX_HOST_ARGS],
    },
    /// 经 [`crate::host::VmHost::register_rule_dispatch`] 注册分派规则。
    RegisterRuleDispatch {
        /// 结果槽（通常 Unit）。
        dst: SlotIndex,
        /// 头符号槽。
        head: SlotIndex,
        /// 扩展算子。
        operator: ExtensionOpId,
        /// pattern 项槽。
        pattern: SlotIndex,
        /// replacement 项槽。
        replacement: SlotIndex,
    },
    /// 经 [`crate::host::VmHost::register_compiled_rule`] 挂接已编译规则。
    RegisterCompiledRule {
        /// 结果槽（通常 Unit）。
        dst: SlotIndex,
        /// 分派表 ID。
        table: u32,
        /// 已编译规则 ID。
        rule: u32,
    },
}

impl Instruction {
    /// 构造一元 `ApplySemantic`。
    pub const fn apply_semantic1(dst: SlotIndex, op: SemanticOpId, arg0: SlotIndex) -> Self {
        let mut args = [0u32; MAX_HOST_ARGS];
        args[0] = arg0;
        Self::ApplySemantic {
            dst,
            op,
            argc: 1,
            args,
        }
    }

    /// 构造二元 `ApplySemantic`。
    pub const fn apply_semantic2(dst: SlotIndex, op: SemanticOpId, arg0: SlotIndex, arg1: SlotIndex) -> Self {
        let mut args = [0u32; MAX_HOST_ARGS];
        args[0] = arg0;
        args[1] = arg1;
        Self::ApplySemantic {
            dst,
            op,
            argc: 2,
            args,
        }
    }

    /// 构造一元 `CallProvider`。
    pub const fn call_provider1(dst: SlotIndex, op: ProviderOpId, arg0: SlotIndex) -> Self {
        let mut args = [0u32; MAX_HOST_ARGS];
        args[0] = arg0;
        Self::CallProvider {
            dst,
            op,
            argc: 1,
            args,
        }
    }
}
