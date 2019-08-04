//! 会话状态变更命令（中性语义 · Living `27` · 非方言表面名）。

use athena_types::{
    BindingEvaluationPolicy, BindingKind, CompiledRuleId, DispatchTableId, SymbolId, TermId,
};

/// Session 级状态变更。定义、清除、导入环境等走此路径，不得伪装成普通 `Application`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// 写入绑定。
    Define {
        /// 被定义符号。
        symbol: SymbolId,
        /// 右值 term。
        value: TermId,
        /// 绑定类别。
        kind: BindingKind,
        /// 求值策略。
        evaluation: BindingEvaluationPolicy,
    },
    /// 注册已编译规则到分派表。
    RegisterRuleDispatch {
        /// 目标分派表。
        table: DispatchTableId,
        /// 已编译规则。
        rule: CompiledRuleId,
    },
    /// 清除符号的 session / residual / dispatch 绑定。
    ClearDefinition {
        /// 目标符号。
        symbol: SymbolId,
    },
}
