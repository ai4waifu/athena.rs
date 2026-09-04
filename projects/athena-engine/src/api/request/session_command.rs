//! 会话状态变更命令（中性语义 · 非方言表面名）。

use athena_types::{SymbolId, TermId};

/// 定义何时求值。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DefinitionEvaluationTiming {
    /// 写入时立即求值右值（对应方言立即赋值一类语义，名称不进本枚举）。
    Immediate,
    /// 使用时再求值右值（对应方言延迟赋值一类语义）。
    Deferred,
}

/// Session 级状态变更。定义、清除、导入环境等走此路径，不得伪装成普通 `Application`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionCommand {
    /// 绑定符号定义。
    Define {
        /// 被定义符号。
        symbol: SymbolId,
        /// 右值 term。
        value: TermId,
        /// 求值时机。
        timing: DefinitionEvaluationTiming,
    },
    /// 清除符号的 Own / Delayed / DownValues。
    ClearDefinition {
        /// 目标符号。
        symbol: SymbolId,
    },
}
