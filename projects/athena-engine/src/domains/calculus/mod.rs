//! 高等数学 — 求导、积分、极限、级数、向量微积分、ODE、变换、留数。
//!
//! 结果为 [`CalculusResult`] / [`ConditionalResult`]，而非无条件裸项。
//! 此处禁止源码文本解析；宿主须传入已解码的 arena [`TermId`]。

pub mod ctx;
mod derivative;
mod differential;
mod integral;
mod limit;
mod request;
mod residue;
mod result;
mod series;
mod symbol_rewrite;
mod transform;
mod value;
mod vector;

pub use ctx::CalculusCtx;
pub use derivative::{differentiate, differentiate_checked};
pub use differential::{DifferentialSolution, VerificationStatus, solve_ode_checked};
pub use integral::{definite_integrate_checked, integrate, integrate_checked};
pub use limit::limit_checked;
pub use request::{CalculusRequest, DerivativeOrder, LimitApproach, LimitDirection, TransformKind};
pub use residue::{Residue, residue_checked};
pub use result::{CalculusResult, ConditionalResult, unresolved, unresolved_from_assumptions};
pub use series::{Remainder, Series, asymptotic, laurent, taylor};
pub use transform::{RegionOfConvergence, TransformResult, fourier_checked, laplace_checked, z_checked};
pub use value::{
    CalculusValue, map_curl_result, map_divergence_result, map_gradient_result, map_hessian_result, map_jacobian_result, map_ode_result,
    map_residue_result, map_series_result, map_term_result, map_transform_result, materialize_calculus_result_term,
};
pub use vector::{
    Curl, Divergence, Gradient, Hessian, Jacobian, curl_checked, divergence_checked, gradient_checked, hessian_checked, jacobian_checked,
};

use athena_types::{Diagnostic, DiagnosticCode};

use crate::runtime::session::Session;

/// 将微积分域请求分派到对应子模块（读写调用方 session arena）。
pub fn execute_calculus(session: &mut Session, request: CalculusRequest) -> CalculusResult<CalculusValue> {
    match request {
        CalculusRequest::Derivative { expression, variable, order, assumptions } => {
            let mut dc = crate::domains::DomainExecutionContext::new(session);
            let times = match order {
                DerivativeOrder::First => 1u32,
                DerivativeOrder::Repeated(n) => n,
            };
            if times == 0 {
                return CalculusResult::Exact { value: CalculusValue::Expression(expression), conditions: Vec::new() };
            }
            let mut value = expression;
            let mut last = differentiate_checked(&mut dc, value, &variable, &assumptions);
            value = dc.fold_term(last.value);
            for _ in 1..times {
                last = differentiate_checked(&mut dc, value, &variable, &assumptions);
                value = dc.fold_term(last.value);
            }
            map_term_result(CalculusResult::from_conditional(ConditionalResult {
                value,
                conditions: last.conditions,
                unresolved: last.unresolved,
            }))
        }
        other => {
            let mut cc = CalculusCtx::new(session);
            match other {
        CalculusRequest::Integral { expression, variable, assumptions: _ } => {
            map_term_result(integrate_checked(&mut cc, expression, &variable))
        }
        CalculusRequest::DefiniteIntegral { expression, variable, lower, upper, assumptions: _ } => {
            map_term_result(definite_integrate_checked(&mut cc, expression, &variable, lower, upper))
        }
        CalculusRequest::Limit { expression, variable, approach, direction, assumptions } => {
            map_term_result(limit_checked(&mut cc, expression, &variable, &approach, direction, &assumptions))
        }
        CalculusRequest::Series { expression, variable, center, order, assumptions: _ } => {
            map_series_result(taylor(&mut cc, expression, &variable, center, order))
        }
        CalculusRequest::Laurent { expression, variable, center, order, assumptions: _ } => {
            map_series_result(laurent(&mut cc, expression, &variable, center, order))
        }
        CalculusRequest::Asymptotic { expression, variable, order, assumptions: _ } => {
            map_series_result(asymptotic(&mut cc, expression, &variable, order))
        }
        CalculusRequest::Residue { expression, variable, point, assumptions: _ } => {
            map_residue_result(residue_checked(&mut cc, expression, &variable, point))
        }
        CalculusRequest::Gradient { expression, variables, assumptions } => {
            map_gradient_result(gradient_checked(&mut cc, expression, &variables, &assumptions))
        }
        CalculusRequest::Jacobian { expressions, variables, assumptions } => {
            map_jacobian_result(jacobian_checked(&mut cc, &expressions, &variables, &assumptions))
        }
        CalculusRequest::Hessian { expression, variables, assumptions } => {
            map_hessian_result(hessian_checked(&mut cc, expression, &variables, &assumptions))
        }
        CalculusRequest::Divergence { components, variables, assumptions } => {
            map_divergence_result(divergence_checked(&mut cc, &components, &variables, &assumptions))
        }
        CalculusRequest::Curl { components, variables, assumptions } => {
            map_curl_result(curl_checked(&mut cc, &components, &variables, &assumptions))
        }
        CalculusRequest::SolveOde { equation, dependent, independent, initial, assumptions } => {
            map_ode_result(solve_ode_checked(&mut cc, equation, &dependent, &independent, initial, &assumptions))
        }
        CalculusRequest::Transform { kind, expression, time_variable, transform_variable, assumptions } => match kind {
            TransformKind::Laplace => {
                map_transform_result(laplace_checked(&mut cc, expression, &time_variable, &transform_variable, &assumptions))
            }
            TransformKind::Fourier => {
                map_transform_result(fourier_checked(&mut cc, expression, &time_variable, &transform_variable, &assumptions))
            }
            TransformKind::Z => map_transform_result(z_checked(&mut cc, expression, &time_variable, &transform_variable, &assumptions)),
        },
        CalculusRequest::Derivative { .. } => unreachable!("derivative handled above"),
            }
        }
    }
}

/// 域尚未接入时的便捷错误。
#[allow(dead_code)]
fn domain_unsupported(_name: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
}
