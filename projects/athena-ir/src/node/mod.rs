//! Core CAS IR term 种类（arena 持有，由 [`TermId`](athena_types::TermId) 引用）。

use athena_numeric::{NumericContext, NumericValue};
use athena_types::{CollectionKind, Result, SourceSpan, SymbolId, TermId};

use crate::operator::ApplicationHead;

/// 原子 term 载荷。
///
/// Living `19`：不实现 [`Clone`]（[`NumericValue`] 无 `Clone`）。深复制用 [`Self::try_clone_in`]。
#[derive(Debug, PartialEq)]
pub enum Atom {
    /// 内核数字（唯一数值真相源：[`NumericValue`]）。
    Number(NumericValue),
    /// 字符串字面量。
    String(String),
    /// intern 符号。
    Symbol(SymbolId),
    /// Typed Boolean。
    Boolean(bool),
    /// Typed Null。
    Null,
}

impl Atom {
    /// Owning 复制：数字经 [`NumericValue::try_clone_in`]。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        Ok(match self {
            Self::Number(n) => Self::Number(n.try_clone_in(ctx)?),
            Self::String(s) => Self::String(s.clone()),
            Self::Symbol(id) => Self::Symbol(*id),
            Self::Boolean(b) => Self::Boolean(*b),
            Self::Null => Self::Null,
        })
    }
}

/// Core IR 中的 term 节点。
///
/// Living `19`：不实现 [`Clone`]。节点经 arena `TermId` 引用；载荷复制用 [`Self::try_clone_in`]。
/// Living `27`：集合必须带显式 [`CollectionKind`]，禁止万能 `List`。
#[derive(Debug, PartialEq)]
pub enum TermNode {
    /// 原子值。
    Atom(Atom),
    /// 显式种类的有序元素集合。
    Collection {
        /// 集合种类。
        kind: CollectionKind,
        /// 元素 term。
        elements: Vec<TermId>,
    },
    /// 算子应用。
    Application {
        /// Semantic or extension head.
        head: ApplicationHead,
        /// 参数 term。
        arguments: Vec<TermId>,
    },
}

impl TermNode {
    /// Owning 复制。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        Ok(match self {
            Self::Atom(a) => Self::Atom(a.try_clone_in(ctx)?),
            Self::Collection { kind, elements } => Self::Collection { kind: *kind, elements: elements.clone() },
            Self::Application { head: op, arguments: args } => Self::Application { head: *op, arguments: args.clone() },
        })
    }

    /// 是否为原子 term。
    pub fn is_atom(&self) -> bool {
        matches!(self, Self::Atom(_))
    }

    /// 默认空 span。
    pub fn default_span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }
}
