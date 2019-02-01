//! 优化与规划领域模块（`athena-engine` 内，非独立 crate）。
//!
//! Living `16`：优化不是 [`crate::solve`] 的别名。本模块拥有问题身份、变量/约束/目标、
//! 可行集、结果状态与证书合同。调度复用 [`crate::solver`]，数值与矩阵复用
//! [`crate::linear_algebra`] / `athena-numeric`。
//!
//! 算法（simplex / branch-and-bound / SQP 等）在语义地基稳定前仅标 bootstrap。
//! 禁止新增 `athena-optimization` 微 crate。
//!
//! 与 [`crate::solve::Constraint`] 不同：本模块的 [`Constraint`] 是优化可行域约束，
//! 请经 `optimization::Constraint` 路径引用，避免与 Solve 约束在 crate 根混淆。

mod certificate;
mod constraint;
mod feasible;
mod fingerprint;
mod frontier;
mod ids;
mod limits;
mod objective;
mod problem;
mod request;
mod result;
mod variable;

pub use certificate::{BoundCertificate, CertificateKind, OptimalityKind};
pub use constraint::{Constraint, ConstraintRelation};
pub use feasible::{ClosureStatus, FeasibleSet};
pub use fingerprint::{FINGERPRINT_ALGORITHM, OptimizationFingerprint, fingerprint_placeholder};
pub use frontier::OptimizationFrontier;
pub use ids::{ConstraintId, ObjectiveId, ProblemId, VariableId};
pub use limits::OptimizationLimits;
pub use objective::{Objective, ObjectiveSense};
pub use problem::{AlgorithmPolicy, OptimizationProblem, ProblemClass};
pub use request::OptimizationRequest;
pub use result::{OptimizationResult, execute_optimization, operation_name};
pub use variable::{DecisionVariable, Integrality, VariableDomain, VariableMetadata};
