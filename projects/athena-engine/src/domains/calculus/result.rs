//! 微积分结果契约 — 表达式 + 条件 + 完备性。

use athena_types::{AssumptionSet, Condition, Diagnostic, ExprId, Predicate};

/// 携带值及其适用条件的结果。
#[derive(Debug, PartialEq)]
pub struct ConditionalResult<T> {
    /// 计算得到的值。
    pub value: T,
    /// `value` 成立所需条件。
    pub conditions: Vec<Condition>,
    /// 引擎未能消解的条件。
    pub unresolved: Vec<Condition>,
}

impl<T> ConditionalResult<T> {
    /// 无条件的精确结果。
    pub fn exact(value: T) -> Self {
        Self { value, conditions: Vec::new(), unresolved: Vec::new() }
    }

    /// 带未消解谓词的结果（调用方不得视为无条件）。
    pub fn with_unresolved(value: T, unresolved: Vec<Condition>) -> Self {
        Self { value, conditions: Vec::new(), unresolved }
    }
}

/// 统一的微积分结果（非裸表达式项）。
#[derive(Debug, PartialEq)]
pub enum CalculusResult<T = ExprId> {
    /// 精确符号结果。
    Exact {
        /// 值。
        value: T,
        /// 已消解条件。
        conditions: Vec<Condition>,
    },
    /// 仅在所列条件下成立的结果。
    Conditional {
        /// 值。
        value: T,
        /// 条件。
        conditions: Vec<Condition>,
    },
    /// 未求值，并附结构化原因。
    Unevaluated {
        /// 原始或残余表达式。
        expression: T,
        /// 求值停止原因。
        reason: Diagnostic,
    },
}

impl<T> CalculusResult<T> {
    /// 将 [`ConditionalResult`] 转为公开枚举。
    pub fn from_conditional(c: ConditionalResult<T>) -> Self {
        if c.unresolved.is_empty() && c.conditions.is_empty() {
            Self::Exact { value: c.value, conditions: Vec::new() }
        }
        else if c.unresolved.is_empty() {
            Self::Conditional { value: c.value, conditions: c.conditions }
        }
        else {
            let mut conditions = c.conditions;
            conditions.extend(c.unresolved.iter().cloned());
            Self::Conditional { value: c.value, conditions }
        }
    }
}

/// 从未充分使用的假设集构建未消解条件。
pub fn unresolved_from_assumptions(set: &AssumptionSet) -> Vec<Condition> {
    set.predicates.iter().cloned().map(|predicate| Condition { predicate, resolved: false }).collect()
}

/// 辅助：将谓词标记为未消解。
pub fn unresolved(predicate: Predicate) -> Condition {
    Condition { predicate, resolved: false }
}
