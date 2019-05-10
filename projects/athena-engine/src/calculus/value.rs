//! 统一的微积分 / 域值（表达式、级数或向量微积分对象）。

use crate::{numeric_clone::clone_term, term::Term};

use super::{
    differential::DifferentialSolution,
    residue::Residue,
    result::CalculusResult,
    series::Series,
    transform::TransformResult,
    vector::{Curl, Divergence, Gradient, Hessian, Jacobian},
};

/// 域 / 微积分响应所携带的值。
#[derive(Debug, PartialEq)]
pub enum CalculusValue {
    /// 普通表达式。
    Expression(Term),
    /// 独立级数对象（保留余项）。
    Series(Series),
    /// 梯度对象（非裸列表）。
    Gradient(Gradient),
    /// Jacobian 矩阵对象。
    Jacobian(Jacobian),
    /// Hessian 矩阵对象。
    Hessian(Hessian),
    /// 散度对象（标量值）。
    Divergence(Divergence),
    /// 旋度对象（三维向量）。
    Curl(Curl),
    /// 复留数对象。
    Residue(Residue),
    /// ODE 解对象（残差已验证）。
    DifferentialSolution(DifferentialSolution),
    /// 带 ROC 的积分变换。
    Transform(TransformResult),
}

impl From<Term> for CalculusValue {
    fn from(value: Term) -> Self {
        Self::Expression(value)
    }
}

impl From<Series> for CalculusValue {
    fn from(value: Series) -> Self {
        Self::Series(value)
    }
}

impl From<Gradient> for CalculusValue {
    fn from(value: Gradient) -> Self {
        Self::Gradient(value)
    }
}

impl From<Jacobian> for CalculusValue {
    fn from(value: Jacobian) -> Self {
        Self::Jacobian(value)
    }
}

impl From<Hessian> for CalculusValue {
    fn from(value: Hessian) -> Self {
        Self::Hessian(value)
    }
}

impl From<Divergence> for CalculusValue {
    fn from(value: Divergence) -> Self {
        Self::Divergence(value)
    }
}

impl From<Curl> for CalculusValue {
    fn from(value: Curl) -> Self {
        Self::Curl(value)
    }
}

impl From<Residue> for CalculusValue {
    fn from(value: Residue) -> Self {
        Self::Residue(value)
    }
}

impl From<DifferentialSolution> for CalculusValue {
    fn from(value: DifferentialSolution) -> Self {
        Self::DifferentialSolution(value)
    }
}

impl From<TransformResult> for CalculusValue {
    fn from(value: TransformResult) -> Self {
        Self::Transform(value)
    }
}

impl CalculusValue {
    /// 展平为桥接 [`Term`]，供仍需要单一表达式的宿主使用。
    pub fn to_bridge_term(&self) -> Term {
        match self {
            Self::Expression(t) => clone_term(t),
            Self::Series(s) => s.to_term(),
            Self::Gradient(g) => g.to_list_term(),
            Self::Jacobian(j) => j.to_list_term(),
            Self::Hessian(h) => h.to_list_term(),
            Self::Divergence(d) => d.to_bridge_term(),
            Self::Curl(c) => c.to_list_term(),
            Self::Residue(r) => r.to_bridge_term(),
            Self::DifferentialSolution(d) => d.to_equal_term(),
            Self::Transform(t) => t.to_bridge_term(),
        }
    }
}

/// 将仅含项的微积分结果映射为值结果。
pub fn map_term_result(r: CalculusResult<Term>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Expression(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Expression(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Expression(expression), reason }
        }
    }
}

/// 将级数微积分结果映射为值结果。
pub fn map_series_result(r: CalculusResult<Series>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Series(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Series(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Series(expression), reason }
        }
    }
}

/// 将类型化向量微积分结果映射为 [`CalculusValue`]。
pub fn map_gradient_result(r: CalculusResult<Gradient>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Gradient(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Gradient(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Gradient(expression), reason }
        }
    }
}

/// 映射 Jacobian 结果。
pub fn map_jacobian_result(r: CalculusResult<Jacobian>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Jacobian(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Jacobian(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Jacobian(expression), reason }
        }
    }
}

/// 映射 Hessian 结果。
pub fn map_hessian_result(r: CalculusResult<Hessian>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Hessian(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Hessian(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Hessian(expression), reason }
        }
    }
}

/// 映射散度结果。
pub fn map_divergence_result(r: CalculusResult<Divergence>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Divergence(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Divergence(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Divergence(expression), reason }
        }
    }
}

/// 映射旋度结果。
pub fn map_curl_result(r: CalculusResult<Curl>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => CalculusResult::Exact { value: CalculusValue::Curl(value), conditions },
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Curl(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Curl(expression), reason }
        }
    }
}

/// 映射留数结果。
pub fn map_residue_result(r: CalculusResult<Residue>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Residue(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Residue(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Residue(expression), reason }
        }
    }
}

/// 映射 ODE 解结果。
pub fn map_ode_result(r: CalculusResult<DifferentialSolution>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::DifferentialSolution(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::DifferentialSolution(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::DifferentialSolution(expression), reason }
        }
    }
}

/// 映射变换结果。
pub fn map_transform_result(r: CalculusResult<TransformResult>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => {
            CalculusResult::Exact { value: CalculusValue::Transform(value), conditions }
        }
        CalculusResult::Conditional { value, conditions } => {
            CalculusResult::Conditional { value: CalculusValue::Transform(value), conditions }
        }
        CalculusResult::Unevaluated { expression, reason } => {
            CalculusResult::Unevaluated { expression: CalculusValue::Transform(expression), reason }
        }
    }
}

/// 抽取 evaluate 风格 API 的主载荷。
pub fn calculus_result_bridge_term(r: &CalculusResult<CalculusValue>) -> Term {
    match r {
        CalculusResult::Exact { value, .. }
        | CalculusResult::Conditional { value, .. }
        | CalculusResult::Unevaluated { expression: value, .. } => value.to_bridge_term(),
    }
}
