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
pub mod plan_exec;
pub mod plan_normalize;
pub mod plan_select;
pub mod planner;
pub mod polynomial;
pub mod solve;
pub mod verify_replay;
pub mod views;

pub use context::DomainExecutionContext;
pub use dispatch::{DomainRequest, DomainResult, execute_domain};
pub use plan_exec::{PlanStepReport, interpret_domain_plan};
pub use planner::{DomainPlan, PlanStep, plan_domain};
pub use verify_replay::{VerifySnapshot, verify_recompute_domain_result};
pub use views::{
    GraphMatrixView, LeaseSet, PolynomialMatrixView, SeriesPolynomialView, TypedViewHeader, ViewFingerprint, ViewKind, ViewRevision,
};
