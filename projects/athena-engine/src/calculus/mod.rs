//! Higher mathematics — derivative, integral, limit, series, vector calculus.
//!
//! Results are [`CalculusResult`] / [`ConditionalResult`], not bare unconditional terms.
//! Source-text parse is forbidden here; hosts pass already-decoded [`Term`] values.

mod derivative;
mod integral;
mod limit;
mod request;
mod result;
mod series;
mod term_util;
mod value;
mod vector;

pub use derivative::{differentiate, differentiate_checked};
pub use integral::{definite_integrate_checked, integrate, integrate_checked};
pub use limit::limit_checked;
pub use request::{CalculusRequest, DerivativeOrder, DomainRequest, LimitApproach, LimitDirection};
pub use result::{CalculusResult, ConditionalResult, unresolved, unresolved_from_assumptions};
pub use series::{Remainder, Series, taylor};
pub use value::{
    CalculusValue, map_gradient_result, map_hessian_result, map_jacobian_result, map_series_result, map_term_result,
};
pub use vector::{
    Gradient, Hessian, Jacobian, gradient_checked, hessian_checked, jacobian_checked,
};

use athena_types::{Diagnostic, DiagnosticCode};

use crate::eval::evaluate;

/// Dispatch a calculus domain request to the appropriate submodule.
pub fn execute_calculus(request: CalculusRequest) -> CalculusResult<CalculusValue> {
    match request {
        CalculusRequest::Derivative {
            expression,
            variable,
            order,
            assumptions,
        } => {
            let times = match order {
                DerivativeOrder::First => 1u32,
                DerivativeOrder::Repeated(n) => n,
            };
            if times == 0 {
                return CalculusResult::Exact {
                    value: CalculusValue::Expression(expression),
                    conditions: Vec::new(),
                };
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
        CalculusRequest::Integral {
            expression,
            variable,
            assumptions: _,
        } => map_term_result(integrate_checked(&expression, &variable)),
        CalculusRequest::DefiniteIntegral {
            expression,
            variable,
            lower,
            upper,
            assumptions: _,
        } => map_term_result(definite_integrate_checked(
            &expression,
            &variable,
            &lower,
            &upper,
        )),
        CalculusRequest::Limit {
            expression,
            variable,
            approach,
            direction,
            assumptions,
        } => map_term_result(limit_checked(
            &expression,
            &variable,
            &approach,
            direction,
            &assumptions,
        )),
        CalculusRequest::Series {
            expression,
            variable,
            center,
            order,
            assumptions: _,
        } => map_series_result(taylor(&expression, &variable, &center, order)),
        CalculusRequest::Gradient {
            expression,
            variables,
            assumptions,
        } => map_gradient_result(gradient_checked(&expression, &variables, &assumptions)),
        CalculusRequest::Jacobian {
            expressions,
            variables,
            assumptions,
        } => map_jacobian_result(jacobian_checked(&expressions, &variables, &assumptions)),
        CalculusRequest::Hessian {
            expression,
            variables,
            assumptions,
        } => map_hessian_result(hessian_checked(&expression, &variables, &assumptions)),
    }
}

/// Dispatch a top-level [`DomainRequest`].
pub fn execute_domain(request: DomainRequest) -> Result<CalculusResult<CalculusValue>, Diagnostic> {
    match request {
        DomainRequest::Calculus(req) => Ok(execute_calculus(req)),
    }
}

/// Convenience error when a domain is not yet wired.
#[allow(dead_code)]
fn domain_unsupported(name: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::UnsupportedOperation,
        format!("domain `{name}` not implemented"),
    )
}
