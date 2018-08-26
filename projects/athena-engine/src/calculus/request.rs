//! Calculus domain requests (stable wire shape for hosts).

use athena_types::AssumptionSet;

use crate::term::Term;

/// Order of differentiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivativeOrder {
    /// First derivative.
    First,
    /// Repeated ordinary derivative.
    Repeated(u32),
}

impl Default for DerivativeOrder {
    fn default() -> Self {
        Self::First
    }
}

/// How a limit approaches its point.
#[derive(Debug, Clone, PartialEq)]
pub enum LimitApproach {
    /// Finite point (already-decoded term, not source text).
    Finite(Term),
    /// +∞.
    PositiveInfinity,
    /// −∞.
    NegativeInfinity,
}

/// Side of a real limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LimitDirection {
    /// Two-sided.
    #[default]
    TwoSided,
    /// From below.
    FromBelow,
    /// From above.
    FromAbove,
}

/// Calculus domain request — hosts map dialect forms here.
#[derive(Debug, Clone, PartialEq)]
pub enum CalculusRequest {
    /// Ordinary / repeated derivative.
    Derivative {
        /// Expression (already decoded).
        expression: Term,
        /// Differentiation variable name (bridge until SymbolId binding).
        variable: String,
        /// Order.
        order: DerivativeOrder,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Limit.
    Limit {
        /// Expression.
        expression: Term,
        /// Variable.
        variable: String,
        /// Approach.
        approach: LimitApproach,
        /// Direction.
        direction: LimitDirection,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Indefinite integral.
    Integral {
        /// Expression.
        expression: Term,
        /// Integration variable.
        variable: String,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Definite integral on a finite interval.
    DefiniteIntegral {
        /// Expression.
        expression: Term,
        /// Integration variable.
        variable: String,
        /// Lower bound (already decoded).
        lower: Term,
        /// Upper bound (already decoded).
        upper: Term,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Taylor / power series about a center.
    Series {
        /// Expression.
        expression: Term,
        /// Expansion variable.
        variable: String,
        /// Center (already decoded).
        center: Term,
        /// Max power included.
        order: u32,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Gradient of a scalar field.
    Gradient {
        /// Scalar expression.
        expression: Term,
        /// Variables in order.
        variables: Vec<String>,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Jacobian of a vector-valued map.
    Jacobian {
        /// Component expressions.
        expressions: Vec<Term>,
        /// Independent variables.
        variables: Vec<String>,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// Hessian of a scalar field.
    Hessian {
        /// Scalar expression.
        expression: Term,
        /// Variables in order (mixed partials keep this order).
        variables: Vec<String>,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
    /// First-order ODE solve (bootstrap subset).
    SolveOde {
        /// Equation term (`Equal[…]`).
        equation: Term,
        /// Dependent variable.
        dependent: String,
        /// Independent variable.
        independent: String,
        /// Assumptions.
        assumptions: AssumptionSet,
    },
}

/// Top-level domain request enum (calculus first; other domains extend later).
#[derive(Debug, Clone, PartialEq)]
pub enum DomainRequest {
    /// Calculus / higher mathematics.
    Calculus(CalculusRequest),
}
