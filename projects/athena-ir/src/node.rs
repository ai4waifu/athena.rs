//! Core CAS IR term 种类（arena 持有，由 [`TermId`](athena_types::TermId) 引用）。

use athena_numeric::{NumericContext, NumericValue};
use athena_types::{OperatorId, Result, SourceSpan, SymbolId, TermId};

/// 原子 term 载荷。
///
/// Living `19`：不实现 [`Clone`]（[`NumericValue`] 无 `Clone`）。深复制用 [`Self::try_clone_in`]。
#[derive(Debug, PartialEq)]
pub enum AtomKind {
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

impl AtomKind {
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
#[derive(Debug, PartialEq)]
pub enum TermKind {
    /// 原子值。
    Atom(AtomKind),
    /// 有序序列（列表 / 向量表面）。
    List(Vec<TermId>),
    /// 算子应用。
    App {
        /// 注册算子。
        op: OperatorId,
        /// 参数 term。
        args: Vec<TermId>,
    },
}

impl TermKind {
    /// Owning 复制。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        Ok(match self {
            Self::Atom(a) => Self::Atom(a.try_clone_in(ctx)?),
            Self::List(xs) => Self::List(xs.clone()),
            Self::App { op, args } => Self::App { op: *op, args: args.clone() },
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
