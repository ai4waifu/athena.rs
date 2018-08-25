//! Vector calculus objects — Gradient, Jacobian, Hessian (not bare lists).

use athena_types::{AssumptionSet, Condition};

use crate::eval::evaluate;
use crate::term::Term;

use super::derivative::differentiate_checked;
use super::result::{CalculusResult, ConditionalResult};

/// Gradient of a scalar field: independent object with ordered components.
#[derive(Debug, Clone, PartialEq)]
pub struct Gradient {
    /// Source scalar expression.
    pub expression: Term,
    /// Variables in differentiation order.
    pub variables: Vec<String>,
    /// ∂f/∂xᵢ components (same order as `variables`).
    pub components: Vec<Term>,
}

impl Gradient {
    /// Bridge list form for hosts that still need a [`Term`] list.
    pub fn to_list_term(&self) -> Term {
        Term::List(self.components.clone())
    }
}

/// Jacobian matrix of a vector-valued map.
#[derive(Debug, Clone, PartialEq)]
pub struct Jacobian {
    /// Component expressions f₁…fₘ.
    pub expressions: Vec<Term>,
    /// Independent variables x₁…xₙ.
    pub variables: Vec<String>,
    /// Rows: `rows[i][j] = ∂fᵢ/∂xⱼ`.
    pub rows: Vec<Vec<Term>>,
}

impl Jacobian {
    /// Nested list term `{{…},…}` bridge.
    pub fn to_list_term(&self) -> Term {
        Term::List(self.rows.iter().map(|r| Term::List(r.clone())).collect())
    }
}

/// Hessian matrix of a scalar field (second partials).
#[derive(Debug, Clone, PartialEq)]
pub struct Hessian {
    /// Source scalar expression.
    pub expression: Term,
    /// Variables in order.
    pub variables: Vec<String>,
    /// `entries[i][j] = ∂²f / ∂xᵢ∂xⱼ` (variable order preserved; no silent swap).
    pub entries: Vec<Vec<Term>>,
}

impl Hessian {
    /// Nested list term bridge.
    pub fn to_list_term(&self) -> Term {
        Term::List(self.entries.iter().map(|r| Term::List(r.clone())).collect())
    }
}

/// ∇f with respect to `variables`.
pub fn gradient_checked(
    expression: &Term,
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Gradient> {
    if variables.is_empty() {
        return CalculusResult::Exact {
            value: Gradient {
                expression: expression.clone(),
                variables: Vec::new(),
                components: Vec::new(),
            },
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
        Gradient {
            expression: expression.clone(),
            variables: variables.to_vec(),
            components,
        },
        conditions,
        unresolved,
    )
}

/// Jacobian of `expressions` w.r.t. `variables`.
pub fn jacobian_checked(
    expressions: &[Term],
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Jacobian> {
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
    finish_vector(
        Jacobian {
            expressions: expressions.to_vec(),
            variables: variables.to_vec(),
            rows,
        },
        conditions,
        unresolved,
    )
}

/// Hessian of a scalar: ∂/∂xᵢ of (∂f/∂xⱼ), keeping variable order.
pub fn hessian_checked(
    expression: &Term,
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Hessian> {
    let mut entries = Vec::with_capacity(variables.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for vi in variables {
        let first = differentiate_checked(expression, vi, assumptions);
        merge_conditions(
            &mut conditions,
            &mut unresolved,
            first.conditions.clone(),
            first.unresolved.clone(),
        );
        let first_val = evaluate(&first.value);
        let mut row = Vec::with_capacity(variables.len());
        for vj in variables {
            // Order: differentiate first w.r.t. vi, then w.r.t. vj (no commute rewrite).
            let second = differentiate_checked(&first_val, vj, assumptions);
            merge_conditions(&mut conditions, &mut unresolved, second.conditions, second.unresolved);
            row.push(evaluate(&second.value));
        }
        entries.push(row);
    }
    finish_vector(
        Hessian {
            expression: expression.clone(),
            variables: variables.to_vec(),
            entries,
        },
        conditions,
        unresolved,
    )
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

fn finish_vector<T>(
    value: T,
    conditions: Vec<Condition>,
    unresolved: Vec<Condition>,
) -> CalculusResult<T> {
    CalculusResult::from_conditional(ConditionalResult {
        value,
        conditions,
        unresolved,
    })
}
