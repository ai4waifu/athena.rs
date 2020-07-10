#![doc = include_str!("readme.md")]

//! 高等数学 — 求导、积分、极限、级数、向量微积分、ODE、变换、留数。
//!
//! 类型化的 `CalculusRequest`（`Goal`）→ `DomainExecutionContext` → `CalculusResult`。
//! 禁止源码文本解析与 `CalculusCtx`。

mod derivative;
mod differential;
mod integral;
mod limit;
mod object_ref;
mod request;
mod residue;
mod result;
pub mod series;
mod symbol_rewrite;
mod transform;
mod value;
mod vector;

pub use derivative::{differentiate, differentiate_checked};
pub use differential::{DifferentialSolution, VerificationStatus, solve_ode_checked};
pub use integral::{definite_integrate_checked, integrate, integrate_checked};
pub use limit::limit_checked;
pub use object_ref::{SeriesObjectStore, SeriesRef};
pub use request::{CalculusRequest, DerivativeOrder, LimitApproach, LimitDirection, TransformKind, calculus_request_identity};
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

use athena_types::{Diagnostic, DiagnosticCode, TermId};

use crate::{domains::context::DomainExecutionContext, runtime::session::Session};

/// 执行失败仅在此边界映为 `Unevaluated`；子模块一律经 [`athena_types::Result`] 传播。
fn map_exec<T>(
    expression: CalculusValue,
    r: athena_types::Result<CalculusResult<T>>,
    map: impl FnOnce(CalculusResult<T>) -> CalculusResult<CalculusValue>,
) -> CalculusResult<CalculusValue> {
    match r {
        Ok(cr) => map(cr),
        Err(reason) => CalculusResult::Unevaluated { expression, reason },
    }
}

fn uneval_expr(expression: TermId, reason: Diagnostic) -> CalculusResult<CalculusValue> {
    CalculusResult::Unevaluated { expression: CalculusValue::Expression(expression), reason }
}

/// 将微积分域请求分派到对应子模块（读写调用方 session arena）。
///
/// 无条件 `Exact` 表达式结果会登记到 Session，供准入热路径免二次重算。
pub fn execute_calculus(session: &mut Session, request: CalculusRequest) -> CalculusResult<CalculusValue> {
    let identity = calculus_request_identity(&request);
    let result = execute_calculus_dispatch(session, request);
    if let CalculusResult::Exact { value: CalculusValue::Expression(term), conditions } = &result {
        if conditions.is_empty() {
            session.remember_trusted_calculus(identity, *term);
        }
    }
    result
}

fn execute_calculus_dispatch(session: &mut Session, request: CalculusRequest) -> CalculusResult<CalculusValue> {
    let mut dc = DomainExecutionContext::new(session);
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
            let mut last = match differentiate_checked(&mut dc, value, variable, &assumptions) {
                Ok(c) => c,
                Err(d) => return uneval_expr(expression, d),
            };
            value = match dc.fold_term(last.value) {
                Ok(v) => v,
                Err(d) => return uneval_expr(expression, d),
            };
            for _ in 1..times {
                last = match differentiate_checked(&mut dc, value, variable, &assumptions) {
                    Ok(c) => c,
                    Err(d) => return uneval_expr(expression, d),
                };
                value = match dc.fold_term(last.value) {
                    Ok(v) => v,
                    Err(d) => return uneval_expr(expression, d),
                };
            }
            map_term_result(CalculusResult::from_conditional(ConditionalResult {
                value,
                conditions: last.conditions,
                unresolved: last.unresolved,
            }))
        }
        CalculusRequest::Integral { expression, variable, assumptions: _ } => {
            map_exec(CalculusValue::Expression(expression), integrate_checked(&mut dc, expression, variable), map_term_result)
        }
        CalculusRequest::DefiniteIntegral { expression, variable, lower, upper, assumptions: _ } => map_exec(
            CalculusValue::Expression(expression),
            definite_integrate_checked(&mut dc, expression, variable, lower, upper),
            map_term_result,
        ),
        CalculusRequest::Limit { expression, variable, approach, direction, assumptions } => map_exec(
            CalculusValue::Expression(expression),
            limit_checked(&mut dc, expression, variable, &approach, direction, &assumptions),
            map_term_result,
        ),
        CalculusRequest::Series { expression, variable, center, order, assumptions: _ } => map_exec(
            CalculusValue::Expression(expression),
            taylor(&mut dc, expression, variable, center, order),
            |r| map_series_result(&mut dc.session_mut().series_objects, r),
        ),
        CalculusRequest::Laurent { expression, variable, center, order, assumptions: _ } => map_exec(
            CalculusValue::Expression(expression),
            laurent(&mut dc, expression, variable, center, order),
            |r| map_series_result(&mut dc.session_mut().series_objects, r),
        ),
        CalculusRequest::Asymptotic { expression, variable, order, assumptions: _ } => map_exec(
            CalculusValue::Expression(expression),
            asymptotic(&mut dc, expression, variable, order),
            |r| map_series_result(&mut dc.session_mut().series_objects, r),
        ),
        CalculusRequest::Residue { expression, variable, point, assumptions: _ } => {
            map_exec(CalculusValue::Expression(expression), residue_checked(&mut dc, expression, variable, point), map_residue_result)
        }
        CalculusRequest::Gradient { expression, variables, assumptions } => map_exec(
            CalculusValue::Expression(expression),
            gradient_checked(&mut dc, expression, &variables, &assumptions),
            map_gradient_result,
        ),
        CalculusRequest::Jacobian { expressions, variables, assumptions } => {
            let echo = expressions.first().copied().unwrap_or_else(|| dc.in_(0));
            map_exec(
                CalculusValue::Expression(echo),
                jacobian_checked(&mut dc, &expressions, &variables, &assumptions),
                map_jacobian_result,
            )
        }
        CalculusRequest::Hessian { expression, variables, assumptions } => {
            map_exec(CalculusValue::Expression(expression), hessian_checked(&mut dc, expression, &variables, &assumptions), map_hessian_result)
        }
        CalculusRequest::Divergence { components, variables, assumptions } => {
            let echo = components.first().copied().unwrap_or_else(|| dc.in_(0));
            map_exec(
                CalculusValue::Expression(echo),
                divergence_checked(&mut dc, &components, &variables, &assumptions),
                map_divergence_result,
            )
        }
        CalculusRequest::Curl { components, variables, assumptions } => {
            let echo = components.first().copied().unwrap_or_else(|| dc.in_(0));
            map_exec(CalculusValue::Expression(echo), curl_checked(&mut dc, &components, &variables, &assumptions), map_curl_result)
        }
        CalculusRequest::SolveOde { equation, dependent, independent, initial, assumptions } => map_exec(
            CalculusValue::Expression(equation),
            solve_ode_checked(&mut dc, equation, dependent, independent, initial, &assumptions),
            map_ode_result,
        ),
        CalculusRequest::Transform { kind, expression, time_variable, transform_variable, assumptions } => {
            let echo = CalculusValue::Expression(expression);
            match kind {
                TransformKind::Laplace => map_exec(
                    echo,
                    laplace_checked(&mut dc, expression, time_variable, transform_variable, &assumptions),
                    map_transform_result,
                ),
                TransformKind::Fourier => map_exec(
                    echo,
                    fourier_checked(&mut dc, expression, time_variable, transform_variable, &assumptions),
                    map_transform_result,
                ),
                TransformKind::Z => {
                    map_exec(echo, z_checked(&mut dc, expression, time_variable, transform_variable, &assumptions), map_transform_result)
                }
            }
        }
    }
}

/// 域尚未接入时的便捷错误。
#[allow(dead_code)]
fn domain_unsupported(_name: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "calculus")
}
