//! Higher mathematics — derivative, integral, limit scaffolding.
//!
//! Results are [`CalculusResult`] / [`ConditionalResult`], not bare unconditional terms.
//! Source-text parse is forbidden here; hosts pass already-decoded [`Term`] values.

mod derivative;
mod integral;
mod limit;
mod request;
mod result;

pub use derivative::{differentiate, differentiate_checked};
pub use integral::{integrate, integrate_checked};
pub use limit::limit_checked;
pub use request::{CalculusRequest, DerivativeOrder, DomainRequest, LimitApproach, LimitDirection};
pub use result::{CalculusResult, ConditionalResult, unresolved, unresolved_from_assumptions};

use athena_types::{Diagnostic, DiagnosticCode};

use crate::eval::evaluate;
use crate::term::Term;

/// Dispatch a calculus domain request to the appropriate submodule.
pub fn execute_calculus(request: CalculusRequest) -> CalculusResult<Term> {
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
                    value: expression,
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
            CalculusResult::from_conditional(ConditionalResult {
                value,
                conditions: last.conditions,
                unresolved: last.unresolved,
            })
        }
        CalculusRequest::Integral {
            expression,
            variable,
            assumptions: _,
        } => integrate_checked(&expression, &variable),
        CalculusRequest::Limit {
            expression,
            variable,
            approach,
            direction,
            assumptions,
        } => limit_checked(&expression, &variable, &approach, direction, &assumptions),
    }
}

/// Dispatch a top-level [`DomainRequest`].
pub fn execute_domain(request: DomainRequest) -> Result<CalculusResult<Term>, Diagnostic> {
    match request {
        DomainRequest::Calculus(req) => Ok(execute_calculus(req)),
    }
}

/// Convenience error when a domain is not yet wired.
#[allow(dead_code)]
fn domain_unsupported(name: &str) -> Diagnostic {
    Diagnostic::error(DiagnosticCode::UnsupportedOperation, format!("domain `{name}` not implemented"))
}
