//! 方程与约束对象。

use athena_types::{SourceSpan, TermId};

use super::{binding::BoundSymbol, domain::SolveDomain};

/// 不等式关系（保留方向，不在构造时压成 `lhs - rhs = 0`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equation {
    /// 左端。
    pub lhs: TermId,
    /// 右端。
    pub rhs: TermId,
    /// 源位置（可选）。
    pub span: Option<SourceSpan>,
}

/// 不等式比较算子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InequalityOp {
    /// `<`
    Less,
    /// `≤`
    LessEqual,
    /// `>`
    Greater,
    /// `≥`
    GreaterEqual,
    /// `≠`
    NotEqual,
}

/// 不等式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inequality {
    /// 左端。
    pub lhs: TermId,
    /// 比较。
    pub op: InequalityOp,
    /// 右端。
    pub rhs: TermId,
    /// 源位置（可选）。
    pub span: Option<SourceSpan>,
}

/// 类型化布尔谓词（Solve 约束侧，非假设集 [`athena_types::Predicate`] 替身）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SolvePredicate {
    /// 谓词项（已是布尔语义的 AthenaIR）。
    pub formula: TermId,
    /// 源位置（可选）。
    pub span: Option<SourceSpan>,
}

/// 约束连接词。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintConnective {
    /// 合取。
    And,
    /// 析取。
    Or,
}

/// 约束集合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstraintSet {
    /// 成员间连接词。
    pub connective: ConstraintConnective,
    /// 成员。
    pub members: Vec<Constraint>,
    /// 源位置（可选）。
    pub span: Option<SourceSpan>,
}

impl ConstraintSet {
    /// 空合取（恒真）。
    pub fn empty_and() -> Self {
        Self { connective: ConstraintConnective::And, members: Vec::new(), span: None }
    }

    /// 合取构造。
    pub fn and(members: Vec<Constraint>) -> Self {
        Self { connective: ConstraintConnective::And, members, span: None }
    }

    /// 析取构造。
    pub fn or(members: Vec<Constraint>) -> Self {
        Self { connective: ConstraintConnective::Or, members, span: None }
    }

    /// 是否无成员。
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }
}

/// 量词。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Quantifier {
    /// ∀
    ForAll,
    /// ∃
    Exists,
    /// 唯一存在。
    Unique,
    /// 不存在。
    None,
}

/// 带量词的约束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantifiedConstraint {
    /// 量词。
    pub quantifier: Quantifier,
    /// 绑定变量（与自由变量分离）。
    pub binders: Vec<BoundSymbol>,
    /// 量词作用域上的域。
    pub domain: SolveDomain,
    /// 量词体。
    pub body: Box<Constraint>,
    /// 源位置（可选）。
    pub span: Option<SourceSpan>,
}

/// 统一约束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    /// 方程。
    Equation(Equation),
    /// 不等式。
    Inequality(Inequality),
    /// 谓词。
    Predicate(SolvePredicate),
    /// 约束集。
    Set(ConstraintSet),
    /// 量词约束。
    Quantified(QuantifiedConstraint),
}

impl Constraint {
    /// 方程便捷构造。
    pub fn equation(lhs: TermId, rhs: TermId) -> Self {
        Self::Equation(Equation { lhs, rhs, span: None })
    }

    /// 不等式便捷构造。
    pub fn inequality(lhs: TermId, op: InequalityOp, rhs: TermId) -> Self {
        Self::Inequality(Inequality { lhs, op, rhs, span: None })
    }

    /// 谓词便捷构造。
    pub fn predicate(formula: TermId) -> Self {
        Self::Predicate(SolvePredicate { formula, span: None })
    }
}
