//! [`TermPattern`]：内部 TRS 模式本体（无 VM / 无方言表面名）。

use athena_types::{SymbolId, TermId};

/// 中性 TRS 模式（内部规则系统本体）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TermPattern {
    /// 匹配任意项。
    Any,
    /// 匹配指定 head / 内建类型名（如 `Integer`）。
    HeadConstraint {
        /// 期望的 head 名或类型名。
        head_name: String,
    },
    /// 命名绑定。
    Bind {
        /// 绑定符号。
        name: SymbolId,
        /// 内层模式。
        inner: Box<TermPattern>,
    },
    /// 与字面项结构相等。
    Exact(TermId),
    /// 有序序列（对应 `List` 结构）。
    Sequence(Vec<TermPattern>),
    /// 应用结构（仅比较参数位置；head 约束由上层显式模式表达）。
    StructuralApplication(Vec<TermPattern>),
}
