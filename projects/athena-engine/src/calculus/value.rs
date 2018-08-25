//! Unified calculus / domain values (expression, series, or vector-calculus objects).

use crate::term::Term;

use super::result::CalculusResult;
use super::series::Series;
use super::vector::{Gradient, Hessian, Jacobian};

/// Value carried by a domain / calculus response.
#[derive(Debug, Clone, PartialEq)]
pub enum CalculusValue {
    /// Ordinary expression.
    Expression(Term),
    /// Independent series object (keeps remainder).
    Series(Series),
    /// Gradient object (not a bare list).
    Gradient(Gradient),
    /// Jacobian matrix object.
    Jacobian(Jacobian),
    /// Hessian matrix object.
    Hessian(Hessian),
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

/// Map a term-only calculus result into a value result.
pub fn map_term_result(r: CalculusResult<Term>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => CalculusResult::Exact {
            value: CalculusValue::Expression(value),
            conditions,
        },
        CalculusResult::Conditional { value, conditions } => CalculusResult::Conditional {
            value: CalculusValue::Expression(value),
            conditions,
        },
        CalculusResult::Unevaluated { expression, reason } => CalculusResult::Unevaluated {
            expression: CalculusValue::Expression(expression),
            reason,
        },
    }
}

/// Map a series calculus result into a value result.
pub fn map_series_result(r: CalculusResult<Series>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => CalculusResult::Exact {
            value: CalculusValue::Series(value),
            conditions,
        },
        CalculusResult::Conditional { value, conditions } => CalculusResult::Conditional {
            value: CalculusValue::Series(value),
            conditions,
        },
        CalculusResult::Unevaluated { expression, reason } => CalculusResult::Unevaluated {
            expression: CalculusValue::Series(expression),
            reason,
        },
    }
}

/// Map a typed vector-calculus result into [`CalculusValue`].
pub fn map_gradient_result(r: CalculusResult<Gradient>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => CalculusResult::Exact {
            value: CalculusValue::Gradient(value),
            conditions,
        },
        CalculusResult::Conditional { value, conditions } => CalculusResult::Conditional {
            value: CalculusValue::Gradient(value),
            conditions,
        },
        CalculusResult::Unevaluated { expression, reason } => CalculusResult::Unevaluated {
            expression: CalculusValue::Gradient(expression),
            reason,
        },
    }
}

/// Map Jacobian result.
pub fn map_jacobian_result(r: CalculusResult<Jacobian>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => CalculusResult::Exact {
            value: CalculusValue::Jacobian(value),
            conditions,
        },
        CalculusResult::Conditional { value, conditions } => CalculusResult::Conditional {
            value: CalculusValue::Jacobian(value),
            conditions,
        },
        CalculusResult::Unevaluated { expression, reason } => CalculusResult::Unevaluated {
            expression: CalculusValue::Jacobian(expression),
            reason,
        },
    }
}

/// Map Hessian result.
pub fn map_hessian_result(r: CalculusResult<Hessian>) -> CalculusResult<CalculusValue> {
    match r {
        CalculusResult::Exact { value, conditions } => CalculusResult::Exact {
            value: CalculusValue::Hessian(value),
            conditions,
        },
        CalculusResult::Conditional { value, conditions } => CalculusResult::Conditional {
            value: CalculusValue::Hessian(value),
            conditions,
        },
        CalculusResult::Unevaluated { expression, reason } => CalculusResult::Unevaluated {
            expression: CalculusValue::Hessian(expression),
            reason,
        },
    }
}
