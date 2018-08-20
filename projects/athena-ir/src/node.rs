//! Core CAS IR term 种类（arena 持有，由 [`TermId`](athena_types::TermId) 引用）。

use athena_types::{Number, OperatorId, SourceSpan, SymbolId, TermId};

/// 原子 term 载荷。
#[derive(Debug, Clone, PartialEq)]
pub enum AtomKind {
    /// 内核数字。
    Number(Number),
    /// 字符串字面量。
    String(String),
    /// intern 符号。
    Symbol(SymbolId),
}

/// Core IR 中的 term 节点。
#[derive(Debug, Clone, PartialEq)]
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
    /// 是否为原子 term。
    pub fn is_atom(&self) -> bool {
        matches!(self, Self::Atom(_))
    }

    /// 默认空 span。
    pub fn default_span() -> SourceSpan {
        SourceSpan { start: 0, end: 0 }
    }
}
