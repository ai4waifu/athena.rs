//! Unified calculus / domain values (expression or series object).

use crate::term::Term;

use super::result::CalculusResult;
use super::series::Series;

/// Value carried by a domain / calculus response.
#[derive(Debug, Clone, PartialEq)]
pub enum CalculusValue {
    /// Ordinary expression.
    Expression(Term),
    /// Independent series object (keeps remainder).
    Series(Series),
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
