//! 数学领域 providers 与域分派。

pub mod algebra;
pub mod calculus;
pub mod context;
pub mod dispatch;
pub mod field;
pub mod galois;
pub mod graph_theory;
pub mod group;
pub mod linear_algebra;
pub mod number_theory;
pub mod optimization;
pub mod planner;
pub mod polynomial;
pub mod solve;
pub mod views;

pub use context::DomainExecutionContext;
pub use dispatch::{DomainRequest, DomainResult, execute_domain};
pub use planner::{DomainPlan, PlanStep, plan_domain};
pub use views::{
    GraphMatrixView, LeaseSet, PolynomialMatrixView, SeriesPolynomialView, TypedViewHeader, ViewFingerprint,
    ViewKind, ViewRevision,
};
