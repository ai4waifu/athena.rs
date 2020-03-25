//! 未经验证的等价候选 —— 绝不是 M-Graph 事实。

use athena_rewriter::{LocalRewriteWitness, RewriteRuleId};
use athena_types::TermId;

use super::ids::EClassId;

/// 局部饱和产生的一条候选等式。
///
/// 必须先通过 Verifier + [`crate::reasoning::mgraph::AdmissionGate`]，才能成为
/// 已接纳关系。持有本值 **不会** 改变 Session / M-Graph。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CandidateEquivalence {
    /// 宿主 [`athena_ir::TermStore`] 中的左项根。
    pub left_term: TermId,
    /// 宿主 `TermStore` 中的右项根。
    pub right_term: TermId,
    /// 左项所在的局部 e-class（诊断 / 抽取用）。
    pub left_class: EClassId,
    /// 右项所在的局部 e-class。
    pub right_class: EClassId,
    /// 产生该候选的规则（非规则驱动时为 `None`）。
    pub rule: Option<RewriteRuleId>,
}

impl CandidateEquivalence {
    /// 当候选来自 [`RewriteRuleId`] 时的局部重写见证。
    pub fn local_witness(&self) -> Option<LocalRewriteWitness> {
        Some(LocalRewriteWitness { rule: self.rule?, subject: self.left_term, produced: self.right_term })
    }
}
