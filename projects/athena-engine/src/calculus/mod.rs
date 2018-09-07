//! 高等数学 — 求导、积分、极限、级数、向量微积分、ODE、变换、留数。
//!
//! 结果为 [`CalculusResult`] / [`ConditionalResult`]，而非无条件裸项。
//! 此处禁止源码文本解析；宿主须传入已解码的 [`Term`]。

mod derivative;
mod differential;
mod integral;
mod limit;
mod lower;
mod request;
mod residue;
mod result;
mod series;
mod term_util;
mod transform;
mod value;
mod vector;

pub use derivative::{differentiate, differentiate_checked};
pub use differential::{DifferentialSolution, VerificationStatus, solve_ode_checked};
pub use integral::{definite_integrate_checked, integrate, integrate_checked};
pub use limit::limit_checked;
pub use lower::try_calculus_request;
pub use request::{CalculusRequest, DerivativeOrder, DomainRequest, LimitApproach, LimitDirection, TransformKind};
pub use residue::{Residue, residue_checked};
pub use result::{CalculusResult, ConditionalResult, unresolved, unresolved_from_assumptions};
pub use series::{Remainder, Series, asymptotic, laurent, taylor};
pub use transform::{RegionOfConvergence, TransformResult, fourier_checked, laplace_checked, z_checked};
pub use value::{
    CalculusValue, calculus_result_bridge_term, map_curl_result, map_divergence_result, map_gradient_result,
    map_hessian_result, map_jacobian_result, map_ode_result, map_residue_result, map_series_result, map_term_result,
    map_transform_result,
};
pub use vector::{
    Curl, Divergence, Gradient, Hessian, Jacobian, curl_checked, divergence_checked, gradient_checked, hessian_checked,
    jacobian_checked,
};

use athena_types::{Diagnostic, DiagnosticCode};

use crate::eval::evaluate;

/// 将微积分域请求分派到对应子模块。
pub fn execute_calculus(request: CalculusRequest) -> CalculusResult<CalculusValue> {
    match request {
        CalculusRequest::Derivative { expression, variable, order, assumptions } => {
            let times = match order {
                DerivativeOrder::First => 1u32,
                DerivativeOrder::Repeated(n) => n,
            };
            if times == 0 {
                return CalculusResult::Exact { value: CalculusValue::Expression(expression), conditions: Vec::new() };
            }
            let mut value = expression;
            let mut last = differentiate_checked(&value, &variable, &assumptions);
            value = evaluate(&last.value);
            for _ in 1..times {
                last = differentiate_checked(&value, &variable, &assumptions);
                value = evaluate(&last.value);
            }
            map_term_result(CalculusResult::from_conditional(ConditionalResult {
                value,
                conditions: last.conditions,
                unresolved: last.unresolved,
            }))
        }
        CalculusRequest::Integral { expression, variable, assumptions: _ } => {
            map_term_result(integrate_checked(&expression, &variable))
        }
        CalculusRequest::DefiniteIntegral { expression, variable, lower, upper, assumptions: _ } => {
            map_term_result(definite_integrate_checked(&expression, &variable, &lower, &upper))
        }
        CalculusRequest::Limit { expression, variable, approach, direction, assumptions } => {
            map_term_result(limit_checked(&expression, &variable, &approach, direction, &assumptions))
        }
        CalculusRequest::Series { expression, variable, center, order, assumptions: _ } => {
            map_series_result(taylor(&expression, &variable, &center, order))
        }
        CalculusRequest::Laurent { expression, variable, center, order, assumptions: _ } => {
            map_series_result(laurent(&expression, &variable, &center, order))
        }
        CalculusRequest::Asymptotic { expression, variable, order, assumptions: _ } => {
            map_series_result(asymptotic(&expression, &variable, order))
        }
        CalculusRequest::Residue { expression, variable, point, assumptions: _ } => {
            map_residue_result(residue_checked(&expression, &variable, &point))
        }
        CalculusRequest::Gradient { expression, variables, assumptions } => {
            map_gradient_result(gradient_checked(&expression, &variables, &assumptions))
        }
        CalculusRequest::Jacobian { expressions, variables, assumptions } => {
            map_jacobian_result(jacobian_checked(&expressions, &variables, &assumptions))
        }
        CalculusRequest::Hessian { expression, variables, assumptions } => {
            map_hessian_result(hessian_checked(&expression, &variables, &assumptions))
        }
        CalculusRequest::Divergence { components, variables, assumptions } => {
            map_divergence_result(divergence_checked(&components, &variables, &assumptions))
        }
        CalculusRequest::Curl { components, variables, assumptions } => {
            map_curl_result(curl_checked(&components, &variables, &assumptions))
        }
        CalculusRequest::SolveOde { equation, dependent, independent, initial, assumptions } => {
            map_ode_result(solve_ode_checked(&equation, &dependent, &independent, initial.as_ref(), &assumptions))
        }
        CalculusRequest::Transform { kind, expression, time_variable, transform_variable, assumptions } => match kind {
            TransformKind::Laplace => {
                map_transform_result(laplace_checked(&expression, &time_variable, &transform_variable, &assumptions))
            }
            TransformKind::Fourier => {
                map_transform_result(fourier_checked(&expression, &time_variable, &transform_variable, &assumptions))
            }
            TransformKind::Z => {
                map_transform_result(z_checked(&expression, &time_variable, &transform_variable, &assumptions))
            }
        },
    }
}

/// 分派顶层 [`DomainRequest`]。
pub fn execute_domain(request: DomainRequest) -> Result<CalculusResult<CalculusValue>, Diagnostic> {
    match request {
        DomainRequest::Calculus(req) => Ok(execute_calculus(req)),
    }
}

/// 域尚未接入时的便捷错误。
#[allow(dead_code)]
fn domain_unsupported(name: &str) -> Diagnostic {
    Diagnostic::error(DiagnosticCode::UnsupportedOperation, format!("域 `{name}` 尚未实现"))
}
