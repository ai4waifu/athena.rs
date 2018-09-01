//! 向量微积分对象 — Gradient、Jacobian、Hessian、Divergence、Curl（非裸列表）。

use athena_types::{AssumptionSet, Condition, Diagnostic, DiagnosticCode};

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

/// 向量场散度：带标量值的独立对象。
#[derive(Debug, Clone, PartialEq)]
pub struct Divergence {
    /// 向量场分量 F₁…Fₙ。
    pub components: Vec<Term>,
    /// 坐标变量（与分量同序，`div = Σ ∂Fᵢ/∂xᵢ`）。
    pub variables: Vec<String>,
    /// 已求值的散度标量。
    pub value: Term,
}

impl Divergence {
    /// 桥接为标量 [`Term`]。
    pub fn to_bridge_term(&self) -> Term {
        self.value.clone()
    }
}

/// 三维向量场旋度：独立对象（bootstrap 仅 ℝ³）。
#[derive(Debug, Clone, PartialEq)]
pub struct Curl {
    /// 输入分量 (Fₓ, Fᵧ, F_z)。
    pub components: Vec<Term>,
    /// 坐标 (x, y, z)。
    pub variables: Vec<String>,
    /// 旋度分量（与 `variables` 同序）。
    pub curl_components: Vec<Term>,
}

impl Curl {
    /// 桥接列表形态。
    pub fn to_list_term(&self) -> Term {
        Term::List(self.curl_components.clone())
    }
}

/// `div F = Σᵢ ∂Fᵢ/∂xᵢ`。分量个数必须等于变量个数。
pub fn divergence_checked(
    components: &[Term],
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Divergence> {
    if components.len() != variables.len() {
        return CalculusResult::Unevaluated {
            expression: Divergence {
                components: components.to_vec(),
                variables: variables.to_vec(),
                value: Term::app(
                    "Divergence",
                    vec![Term::List(components.to_vec()), Term::List(variables.iter().map(Term::symbol).collect())],
                ),
            },
            reason: Diagnostic::error(
                DiagnosticCode::UnsupportedOperation,
                format!(
                    "散度要求分量与变量个数相同，得到 {} 与 {}",
                    components.len(),
                    variables.len()
                ),
            ),
        };
    }
    if components.is_empty() {
        return CalculusResult::Exact {
            value: Divergence { components: Vec::new(), variables: Vec::new(), value: Term::int(0) },
            conditions: Vec::new(),
        };
    }
    let mut parts = Vec::with_capacity(components.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for (comp, var) in components.iter().zip(variables.iter()) {
        let part = differentiate_checked(comp, var, assumptions);
        merge_conditions(&mut conditions, &mut unresolved, part.conditions, part.unresolved);
        parts.push(evaluate(&part.value));
    }
    let value = if parts.len() == 1 {
        parts.pop().unwrap()
    } else {
        evaluate(&Term::app("Plus", parts))
    };
    finish_vector(
        Divergence { components: components.to_vec(), variables: variables.to_vec(), value },
        conditions,
        unresolved,
    )
}

/// ℝ³ 旋度：`∇×F = (∂F_z/∂y−∂F_y/∂z, ∂F_x/∂z−∂F_z/∂x, ∂F_y/∂x−∂F_x/∂y)`。
pub fn curl_checked(components: &[Term], variables: &[String], assumptions: &AssumptionSet) -> CalculusResult<Curl> {
    if components.len() != 3 || variables.len() != 3 {
        return CalculusResult::Unevaluated {
            expression: Curl {
                components: components.to_vec(),
                variables: variables.to_vec(),
                curl_components: Vec::new(),
            },
            reason: Diagnostic::error(
                DiagnosticCode::UnsupportedOperation,
                format!(
                    "旋度 bootstrap 仅支持三维，得到分量 {}、变量 {}",
                    components.len(),
                    variables.len()
                ),
            ),
        };
    }
    let fx = &components[0];
    let fy = &components[1];
    let fz = &components[2];
    let x = &variables[0];
    let y = &variables[1];
    let z = &variables[2];
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();

    let d_fz_dy = differentiate_checked(fz, y, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fz_dy.conditions, d_fz_dy.unresolved);
    let d_fy_dz = differentiate_checked(fy, z, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fy_dz.conditions, d_fy_dz.unresolved);

    let d_fx_dz = differentiate_checked(fx, z, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fx_dz.conditions, d_fx_dz.unresolved);
    let d_fz_dx = differentiate_checked(fz, x, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fz_dx.conditions, d_fz_dx.unresolved);

    let d_fy_dx = differentiate_checked(fy, x, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fy_dx.conditions, d_fy_dx.unresolved);
    let d_fx_dy = differentiate_checked(fx, y, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fx_dy.conditions, d_fx_dy.unresolved);

    let cx = sub_terms(&evaluate(&d_fz_dy.value), &evaluate(&d_fy_dz.value));
    let cy = sub_terms(&evaluate(&d_fx_dz.value), &evaluate(&d_fz_dx.value));
    let cz = sub_terms(&evaluate(&d_fy_dx.value), &evaluate(&d_fx_dy.value));

    finish_vector(
        Curl {
            components: components.to_vec(),
            variables: variables.to_vec(),
            curl_components: vec![cx, cy, cz],
        },
        conditions,
        unresolved,
    )
}

fn sub_terms(a: &Term, b: &Term) -> Term {
    evaluate(&Term::app("Plus", vec![a.clone(), Term::app("Times", vec![Term::int(-1), b.clone()])]))
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
