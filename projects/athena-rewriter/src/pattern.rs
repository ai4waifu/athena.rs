//! [`TermPattern`]：内部 TRS 模式本体（Living `27` · 无方言表面名 · 无字符串 head）。

use athena_ir::ApplicationHead;
use athena_types::{CollectionKind, DomainId, PredicateId, SymbolId, TermId, ValueTypeId};

/// 模式约束（闭集身份，禁止 `head_name: String`）。
///
/// Living `31`：纯身份枚举，实现 [`Copy`]。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatternConstraint {
    /// 算子身份（semantic or extension head）。
    Operator(ApplicationHead),
    /// 值类型。
    ValueType(ValueTypeId),
    /// 集合种类。
    CollectionKind(CollectionKind),
    /// 领域身份。
    Domain(DomainId),
    /// 谓词身份。
    Predicate(PredicateId),
}

/// 中性 TRS 模式（内部规则系统本体）。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub enum TermPattern {
    /// 匹配任意项。
    Any,
    /// 命名绑定。
    Bind {
        /// 绑定符号。
        name: SymbolId,
        /// 内层模式。
        inner: Box<TermPattern>,
    },
    /// 与字面项结构相等。
    Exact(TermId),
    /// 有序序列（对应 `Collection` 结构）。
    Sequence(Vec<TermPattern>),
    /// 带算子身份的应用。
    Application {
        /// Semantic or extension head.
        operator: ApplicationHead,
        /// 参数模式。
        arguments: Vec<TermPattern>,
    },
    /// 仅比较参数位置的应用结构（算子约束由 [`Self::Constrained`] 表达）。
    StructuralApplication(Vec<TermPattern>),
    /// 附加约束。
    Constrained {
        /// 内层模式。
        pattern: Box<TermPattern>,
        /// 约束。
        constraint: PatternConstraint,
    },
}

impl TermPattern {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        match self {
            Self::Any => Self::Any,
            Self::Bind { name, inner } => Self::Bind { name: *name, inner: Box::new(inner.owning_copy()) },
            Self::Exact(id) => Self::Exact(*id),
            Self::Sequence(items) => Self::Sequence(items.iter().map(Self::owning_copy).collect()),
            Self::Application { operator, arguments } => {
                Self::Application { operator: *operator, arguments: arguments.iter().map(Self::owning_copy).collect() }
            }
            Self::StructuralApplication(arguments) => {
                Self::StructuralApplication(arguments.iter().map(Self::owning_copy).collect())
            }
            Self::Constrained { pattern, constraint } => {
                Self::Constrained { pattern: Box::new(pattern.owning_copy()), constraint: *constraint }
            }
        }
    }
}
