//! 向量微积分对象 — Gradient、Jacobian、Hessian、Divergence、Curl（arena 版 · Living `25`）。

use athena_ir::SemanticOperator;
use athena_types::{AssumptionSet, Condition, Diagnostic, DiagnosticCode, TermId};

use super::{
    ctx::CalculusCtx,
    derivative::differentiate_checked,
    result::{CalculusResult, ConditionalResult},
};

/// 标量场梯度：带有序分量的独立对象。
#[derive(Debug, PartialEq)]
pub struct Gradient {
    /// 源标量表达式。
    pub expression: TermId,
    /// 求导变量顺序。
    pub variables: Vec<String>,
    /// ∂f/∂xᵢ 分量（与 `variables` 同序）。
    pub components: Vec<TermId>,
}

impl Gradient {
    /// 桥接列表形态，供仍需要列表的宿主。
    pub fn materialize_list_expression(&self, cc: &mut CalculusCtx<'_>) -> TermId {
        cc.list(self.components.clone())
    }
}

/// 向量值映射的 Jacobian 矩阵。
#[derive(Debug, PartialEq)]
pub struct Jacobian {
    /// 分量表达式 f₁…fₘ。
    pub expressions: Vec<TermId>,
    /// 自变量 x₁…xₙ。
    pub variables: Vec<String>,
    /// 行：`rows[i][j] = ∂fᵢ/∂xⱼ`。
    pub rows: Vec<Vec<TermId>>,
}

impl Jacobian {
    /// 嵌套列表项 `{{…},…}` 桥接。
    pub fn materialize_list_expression(&self, cc: &mut CalculusCtx<'_>) -> TermId {
        let rows = self.rows.iter().map(|r| cc.list(r.clone())).collect();
        cc.list(rows)
    }
}

/// 标量场 Hessian 矩阵（二阶偏导）。
#[derive(Debug, PartialEq)]
pub struct Hessian {
    /// 源标量表达式。
    pub expression: TermId,
    /// 按序变量。
    pub variables: Vec<String>,
    /// `entries[i][j] = ∂²f / ∂xᵢ∂xⱼ`（保持变量顺序；不静默交换）。
    pub entries: Vec<Vec<TermId>>,
}

impl Hessian {
    /// 嵌套列表项桥接。
    pub fn materialize_list_expression(&self, cc: &mut CalculusCtx<'_>) -> TermId {
        let rows = self.entries.iter().map(|r| cc.list(r.clone())).collect();
        cc.list(rows)
    }
}

/// 关于 `variables` 的 ∇f。
pub fn gradient_checked(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Gradient> {
    if variables.is_empty() {
        return CalculusResult::Exact { value: Gradient { expression, variables: Vec::new(), components: Vec::new() }, conditions: Vec::new() };
    }
    let mut components = Vec::with_capacity(variables.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for v in variables {
        let part = differentiate_checked(cc, expression, v, assumptions);
        merge_conditions(&mut conditions, &mut unresolved, part.conditions, part.unresolved);
        components.push(cc.eval(part.value));
    }
    finish_vector(Gradient { expression, variables: variables.to_vec(), components }, conditions, unresolved)
}

/// `expressions` 关于 `variables` 的 Jacobian。
pub fn jacobian_checked(
    cc: &mut CalculusCtx<'_>,
    expressions: &[TermId],
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Jacobian> {
    let mut rows = Vec::with_capacity(expressions.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for expr in expressions {
        let mut row = Vec::with_capacity(variables.len());
        for v in variables {
            let part = differentiate_checked(cc, *expr, v, assumptions);
            merge_conditions(&mut conditions, &mut unresolved, part.conditions, part.unresolved);
            row.push(cc.eval(part.value));
        }
        rows.push(row);
    }
    finish_vector(Jacobian { expressions: expressions.to_vec(), variables: variables.to_vec(), rows }, conditions, unresolved)
}

/// 标量 Hessian：先 ∂/∂xᵢ 再对 (∂f/∂xⱼ)，保持变量顺序。
pub fn hessian_checked(
    cc: &mut CalculusCtx<'_>,
    expression: TermId,
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Hessian> {
    let mut entries = Vec::with_capacity(variables.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for vi in variables {
        let first = differentiate_checked(cc, expression, vi, assumptions);
        merge_conditions(&mut conditions, &mut unresolved, first.conditions.clone(), first.unresolved.clone());
        let first_val = cc.eval(first.value);
        let mut row = Vec::with_capacity(variables.len());
        for vj in variables {
            // 顺序：先对 vi 求导，再对 vj（不做交换改写）。
            let second = differentiate_checked(cc, first_val, vj, assumptions);
            merge_conditions(&mut conditions, &mut unresolved, second.conditions, second.unresolved);
            row.push(cc.eval(second.value));
        }
        entries.push(row);
    }
    finish_vector(Hessian { expression, variables: variables.to_vec(), entries }, conditions, unresolved)
}

/// 向量场散度：带标量值的独立对象。
#[derive(Debug, PartialEq)]
pub struct Divergence {
    /// 向量场分量 F₁…Fₙ。
    pub components: Vec<TermId>,
    /// 坐标变量（与分量同序，`div = Σ ∂Fᵢ/∂xᵢ`）。
    pub variables: Vec<String>,
    /// 已求值的散度标量。
    pub value: TermId,
}

impl Divergence {
    /// 桥接为标量项。
    pub fn materialize_expression(&self) -> TermId {
        self.value
    }
}

/// 三维向量场旋度：独立对象（引导实现仅 ℝ³）。
#[derive(Debug, PartialEq)]
pub struct Curl {
    /// 输入分量 (Fₓ, Fᵧ, F_z)。
    pub components: Vec<TermId>,
    /// 坐标 (x, y, z)。
    pub variables: Vec<String>,
    /// 旋度分量（与 `variables` 同序）。
    pub curl_components: Vec<TermId>,
}

impl Curl {
    /// 桥接列表形态。
    pub fn materialize_list_expression(&self, cc: &mut CalculusCtx<'_>) -> TermId {
        cc.list(self.curl_components.clone())
    }
}

/// `div F = Σᵢ ∂Fᵢ/∂xᵢ`。分量个数必须等于变量个数。
pub fn divergence_checked(
    cc: &mut CalculusCtx<'_>,
    components: &[TermId],
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Divergence> {
    if components.len() != variables.len() {
        let comps = cc.list(components.to_vec());
        let vars = cc.list(variables.iter().map(|v| cc.symbol(v)).collect());
        return CalculusResult::Unevaluated {
            expression: Divergence {
                components: components.to_vec(),
                variables: variables.to_vec(),
                value: cc.apply_semantic(SemanticOperator::Divergence, vec![comps, vars]),
            },
            reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation),
        };
    }
    if components.is_empty() {
        return CalculusResult::Exact {
            value: Divergence { components: Vec::new(), variables: Vec::new(), value: cc.in_(0) },
            conditions: Vec::new(),
        };
    }
    let mut parts = Vec::with_capacity(components.len());
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();
    for (comp, var) in components.iter().zip(variables.iter()) {
        let part = differentiate_checked(cc, *comp, var, assumptions);
        merge_conditions(&mut conditions, &mut unresolved, part.conditions, part.unresolved);
        parts.push(cc.eval(part.value));
    }
    let value = if parts.len() == 1 { parts[0] } else { cc.eval(cc.apply_semantic(SemanticOperator::Add, parts)) };
    finish_vector(Divergence { components: components.to_vec(), variables: variables.to_vec(), value }, conditions, unresolved)
}

/// ℝ³ 旋度：`∇×F = (∂F_z/∂y−∂F_y/∂z, ∂F_x/∂z−∂F_z/∂x, ∂F_y/∂x−∂F_x/∂y)`。
pub fn curl_checked(
    cc: &mut CalculusCtx<'_>,
    components: &[TermId],
    variables: &[String],
    assumptions: &AssumptionSet,
) -> CalculusResult<Curl> {
    if components.len() != 3 || variables.len() != 3 {
        return CalculusResult::Unevaluated {
            expression: Curl { components: components.to_vec(), variables: variables.to_vec(), curl_components: Vec::new() },
            reason: Diagnostic::new(DiagnosticCode::UnsupportedOperation),
        };
    }
    let (fx, fy, fz) = (components[0], components[1], components[2]);
    let (x, y, z) = (&variables[0], &variables[1], &variables[2]);
    let mut conditions = Vec::new();
    let mut unresolved = Vec::new();

    let d_fz_dy = differentiate_checked(cc, fz, y, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fz_dy.conditions, d_fz_dy.unresolved);
    let d_fy_dz = differentiate_checked(cc, fy, z, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fy_dz.conditions, d_fy_dz.unresolved);

    let d_fx_dz = differentiate_checked(cc, fx, z, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fx_dz.conditions, d_fx_dz.unresolved);
    let d_fz_dx = differentiate_checked(cc, fz, x, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fz_dx.conditions, d_fz_dx.unresolved);

    let d_fy_dx = differentiate_checked(cc, fy, x, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fy_dx.conditions, d_fy_dx.unresolved);
    let d_fx_dy = differentiate_checked(cc, fx, y, assumptions);
    merge_conditions(&mut conditions, &mut unresolved, d_fx_dy.conditions, d_fx_dy.unresolved);

    let cx = sub_terms(cc, cc.eval(d_fz_dy.value), cc.eval(d_fy_dz.value));
    let cy = sub_terms(cc, cc.eval(d_fx_dz.value), cc.eval(d_fz_dx.value));
    let cz = sub_terms(cc, cc.eval(d_fy_dx.value), cc.eval(d_fx_dy.value));

    finish_vector(
        Curl { components: components.to_vec(), variables: variables.to_vec(), curl_components: vec![cx, cy, cz] },
        conditions,
        unresolved,
    )
}

fn sub_terms(cc: &mut CalculusCtx<'_>, a: TermId, b: TermId) -> TermId {
    let neg = cc.apply_semantic(SemanticOperator::Multiply, vec![cc.in_(-1), b]);
    cc.eval(cc.apply_semantic(SemanticOperator::Add, vec![a, neg]))
}

fn merge_conditions(conditions: &mut Vec<Condition>, unresolved: &mut Vec<Condition>, more_c: Vec<Condition>, more_u: Vec<Condition>) {
    conditions.extend(more_c);
    unresolved.extend(more_u);
}

fn finish_vector<T>(value: T, conditions: Vec<Condition>, unresolved: Vec<Condition>) -> CalculusResult<T> {
    CalculusResult::from_conditional(ConditionalResult { value, conditions, unresolved })
}
