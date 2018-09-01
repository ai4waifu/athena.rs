//! 向量微积分对象 — Gradient、Jacobian、Hessian（非裸列表）。

use athena_types::{AssumptionSet, Condition};

use crate::{eval::evaluate, term::Term};

use super::{
    derivative::differentiate_checked,
    result::{CalculusResult, ConditionalResult},
};

/// 标量场梯度：带有序分量的独立对象。
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// 源标量表达式。
    pub expression: Term,
    /// 求导变量顺序。
    pub variables: Vec<String>,
    /// ∂f/∂xᵢ 分量（与 `variables` 同序）。
    pub components: Vec<Term>,
}

impl Gradient {
    /// 桥接列表形态，供仍需要 [`Term`] 列表的宿主。
    pub fn to_list_term(&self) -> Term {
        Term::List(self.components.clone())
    }
}

/// 向量值映射的 Jacobian 矩阵。
#[derive(Debug, Clone, PartialEq)]
pub struct Jacobian {
    /// 分量表达式 f₁…fₘ。
    pub expressions: Vec<Term>,
    /// 自变量 x₁…xₙ。
    pub variables: Vec<String>,
    /// 行：`rows[i][j] = ∂fᵢ/∂xⱼ`。
    pub rows: Vec<Vec<Term>>,
}

impl Jacobian {
    /// 嵌套列表项 `{{…},…}` 桥接。
    pub fn to_list_term(&self) -> Term {
        Term::List(self.rows.iter().map(|r| Term::List(r.clone())).collect())
    }
}

/// 标量场 Hessian 矩阵（二阶偏导）。
#[derive(Debug, Clone, PartialEq)]
pub struct Hessian {
    /// 源标量表达式。
    pub expression: Term,
    /// 按序变量。
    pub variables: Vec<String>,
    /// `entries[i][j] = ∂²f / ∂xᵢ∂xⱼ`（保持变量顺序；不静默交换）。
    pub entries: Vec<Vec<Term>>,
}

impl Hessian {
    /// 嵌套列表项桥接。
    pub fn to_list_term(&self) -> Term {
        Term::List(self.entries.iter().map(|r| Term::List(r.clone())).collect())
    }
}

/// 关于 `variables` 的 ∇f。
pub fn gradient_checked(expression: &Term, variables: &[String], assumptions: &AssumptionSet) -> CalculusResult<Gradient> {
    if variables.is_empty() {
        return CalculusResult::Exact {
            value: Gradient { expression: expression.clone(), variables: Vec::new(), components: Vec::new() },
            conditions: Vec::new(),
        };
    }
    let mut components = Vec::with_capacity(variables.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for v in variables {
        let part = differentiate_checked(expression, v, assumptions);
        merge_conditions(&mut conditions, &mut unresolved, part.conditions, part.unresolved);
        components.push(evaluate(&part.value));
    }
    finish_vector(
        Gradient { expression: expression.clone(), variables: variables.to_vec(), components },
        conditions,
        unresolved,
    )
}

/// `expressions` 关于 `variables` 的 Jacobian。
pub fn jacobian_checked(expressions: &[Term], variables: &[String], assumptions: &AssumptionSet) -> CalculusResult<Jacobian> {
    let mut rows = Vec::with_capacity(expressions.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for expr in expressions {
        let mut row = Vec::with_capacity(variables.len());
        for v in variables {
            let part = differentiate_checked(expr, v, assumptions);
            merge_conditions(&mut conditions, &mut unresolved, part.conditions, part.unresolved);
            row.push(evaluate(&part.value));
        }
        rows.push(row);
    }
    finish_vector(Jacobian { expressions: expressions.to_vec(), variables: variables.to_vec(), rows }, conditions, unresolved)
}

/// 标量 Hessian：先 ∂/∂xᵢ 再对 (∂f/∂xⱼ)，保持变量顺序。
pub fn hessian_checked(expression: &Term, variables: &[String], assumptions: &AssumptionSet) -> CalculusResult<Hessian> {
    let mut entries = Vec::with_capacity(variables.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for vi in variables {
        let first = differentiate_checked(expression, vi, assumptions);
        merge_conditions(&mut conditions, &mut unresolved, first.conditions.clone(), first.unresolved.clone());
        let first_val = evaluate(&first.value);
        let mut row = Vec::with_capacity(variables.len());
        for vj in variables {
            // 顺序：先对 vi 求导，再对 vj（不做交换改写）。
            let second = differentiate_checked(&first_val, vj, assumptions);
            merge_conditions(&mut conditions, &mut unresolved, second.conditions, second.unresolved);
            row.push(evaluate(&second.value));
        }
        entries.push(row);
    }
    finish_vector(Hessian { expression: expression.clone(), variables: variables.to_vec(), entries }, conditions, unresolved)
}

fn merge_conditions(
    conditions: &mut Vec<Condition>,
    unresolved: &mut Vec<Condition>,
    more_c: Vec<Condition>,
    more_u: Vec<Condition>,
) {
    conditions.extend(more_c);
    unresolved.extend(more_u);
}

fn finish_vector<T>(value: T, conditions: Vec<Condition>, unresolved: Vec<Condition>) -> CalculusResult<T> {
    CalculusResult::from_conditional(ConditionalResult { value, conditions, unresolved })
}
