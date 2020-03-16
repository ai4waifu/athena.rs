//! 优化约束（可行域侧）。
//!
//! 与 [`crate::domains::solve::Constraint`] 分离：Solve 约束描述方程/不等式求解问题；
//! 本类型描述优化可行域成员关系。

use athena_types::{DomainId, TermId};

use super::ids::ConstraintId;

/// 约束关系。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintRelation {
    /// 等式。
    Equal,
    /// ≤。
    LessEqual,
    /// ≥。
    GreaterEqual,
    /// 锥成员（二阶锥 / SDP 等，具体锥由表达式与域刻画）。
    ConeMembership,
    /// 逻辑约束（indicator / SOS1 等，骨架占位）。
    Logical,
}

/// 优化约束。
///
/// Living `31`：**不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct Constraint {
    /// Session-local id。
    pub id: ConstraintId,
    /// 关系。
    pub relation: ConstraintRelation,
    /// 约束表达式（规范形待后续冻结）。
    pub expression: TermId,
    /// 系数 / 嵌入域。
    pub domain: DomainId,
    /// 来源说明（非证书）。
    pub provenance: Option<String>,
}

impl Constraint {
    /// Owning 复制（Living `31`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            id: self.id,
            relation: self.relation,
            expression: self.expression,
            domain: self.domain,
            provenance: self.provenance.clone(),
        }
    }
}
