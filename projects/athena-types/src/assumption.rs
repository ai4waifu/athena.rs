//! 微积分与域条件结果所用的假设集。

use crate::ids::{AssumptionSetId, ExprId, SymbolId};

/// 原子假设谓词（语言中立）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    /// `lhs = rhs`。
    Equal(ExprId, ExprId),
    /// `lhs ≠ rhs`。
    NotEqual(ExprId, ExprId),
    /// `lhs < rhs`。
    Less(ExprId, ExprId),
    /// `lhs ≤ rhs`。
    LessEqual(ExprId, ExprId),
    /// `lhs > rhs`。
    Greater(ExprId, ExprId),
    /// `lhs ≥ rhs`。
    GreaterEqual(ExprId, ExprId),
    /// 值为整数。
    Integer(ExprId),
    /// 值严格为正。
    Positive(ExprId),
    /// 值非负。
    NonNegative(ExprId),
    /// 值为实数。
    Real(ExprId),
    /// 值为复数。
    Complex(ExprId),
    /// 值非零。
    NonZero(ExprId),
    /// 符号非零（桥接，直至 ExprId 绑定落地）。
    SymbolNonZero(SymbolId),
    /// 符号为实数。
    SymbolReal(SymbolId),
}

/// 附着于请求或结果的有序谓词集合。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AssumptionSet {
    /// 存入 Session 注册表时的稳定 id。
    pub id: Option<AssumptionSetId>,
    /// 谓词列表。
    pub predicates: Vec<Predicate>,
}

impl AssumptionSet {
    /// 空假设集。
    pub fn empty() -> Self {
        Self::default()
    }

    /// 由谓词列表构建。
    pub fn from_predicates(predicates: Vec<Predicate>) -> Self {
        Self { id: None, predicates }
    }

    /// 集合是否为空。
    pub fn is_empty(&self) -> bool {
        self.predicates.is_empty()
    }
}

/// 限定微积分（或域）结果适用性的条件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Condition {
    /// `value` 有效时必须成立的谓词。
    pub predicate: Predicate,
    /// 该条件是否已被引擎消解。
    pub resolved: bool,
}
