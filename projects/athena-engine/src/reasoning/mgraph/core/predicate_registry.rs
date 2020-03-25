//! 封闭谓词注册表。
//!
//! 描述符记录每个 [`PredicateId`] 的理论上下文与主体元数。
//! 接纳与超边暂存必须查阅本表 —— 绝不可发明字符串标签。

use std::ops::RangeInclusive;

use super::refs::{PredicateId, TheoryContextId, predicates};

/// 单个封闭 [`PredicateId`] 的静态描述符。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PredicateDescriptor {
    /// 谓词标识。
    pub id: PredicateId,
    /// 所属理论上下文。
    pub theory: TheoryContextId,
    /// 主体元数闭区间（[`crate::reasoning::mgraph::RelationRecord`] 上的 `SemanticRef` 个数）。
    pub subject_arity: RangeInclusive<usize>,
}

const DESCRIPTORS: &[PredicateDescriptor] = &[
    PredicateDescriptor { id: predicates::POLYNOMIAL_RESULT, theory: TheoryContextId::POLYNOMIAL, subject_arity: 1..=1 },
    PredicateDescriptor { id: predicates::CONGRUENCE, theory: TheoryContextId::CONGRUENCE, subject_arity: 3..=3 },
    PredicateDescriptor { id: predicates::REWRITE_EQUIVALENT, theory: TheoryContextId::REWRITE, subject_arity: 2..=2 },
    PredicateDescriptor { id: predicates::EVALUATION_RESULT, theory: TheoryContextId::DEFAULT, subject_arity: 2..=2 },
    PredicateDescriptor { id: predicates::DERIVATIVE_OF, theory: TheoryContextId::CALCULUS, subject_arity: 3..=3 },
    PredicateDescriptor { id: predicates::SERIES_EXPANSION, theory: TheoryContextId::CALCULUS, subject_arity: 3..=3 },
    PredicateDescriptor { id: predicates::INTEGRAL_OF, theory: TheoryContextId::CALCULUS, subject_arity: 3..=3 },
];

/// 查找封闭谓词描述符。
pub fn descriptor(id: PredicateId) -> Option<&'static PredicateDescriptor> {
    DESCRIPTORS.iter().find(|d| d.id == id)
}

/// `subject_count` 对 `id` 是否合法。
pub fn arity_ok(id: PredicateId, subject_count: usize) -> bool {
    descriptor(id).is_some_and(|d| d.subject_arity.contains(&subject_count))
}

/// 全部已注册描述符（按 [`PredicateId`] 稳定排序）。
pub fn all_descriptors() -> &'static [PredicateDescriptor] {
    DESCRIPTORS
}
