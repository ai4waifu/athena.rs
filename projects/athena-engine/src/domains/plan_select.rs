//! `DomainPlan` 的 `SelectRepresentation`。
//!
//! 记录请求当前活跃的 `DomainObject` / 项表示族。
//! 引导期：确认每个领域唯一支持的表示族（无静默算法分支）。
//! 日后规划器可在此于竞争表示之间择优。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        dispatch::DomainRequest,
        linear_algebra::LinearAlgebraRequest,
        polynomial::{PolynomialRequest, refs_from_request},
    },
    runtime::session::Session,
};

/// 供审计 / 恢复用的所选表示标签（稳定字符串 · 非方言名）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectedRepresentation {
    /// 机器可读的表示族 id。
    pub family: &'static str,
}

/// 相对 `Session` 存储为 `request` 选择 / 确认表示。
pub fn select_domain_representation(session: &Session, request: &DomainRequest) -> Result<SelectedRepresentation, Diagnostic> {
    match request {
        DomainRequest::Polynomial(req) => select_polynomial(session, req),
        DomainRequest::LinearAlgebra(req) => select_linear_algebra(session, req),
        DomainRequest::Calculus(_) => Ok(SelectedRepresentation { family: "term_store" }),
        DomainRequest::NumberTheory(_) => Ok(SelectedRepresentation { family: "integer_numeric" }),
        DomainRequest::GroupTheory(_) => Ok(SelectedRepresentation { family: "permutation_presentation" }),
        DomainRequest::FieldTheory(_) => Ok(SelectedRepresentation { family: "field_presentation" }),
        DomainRequest::GaloisTheory(_) => Ok(SelectedRepresentation { family: "extension_presentation" }),
        DomainRequest::GraphTheory(_) => Ok(SelectedRepresentation { family: "graph_object" }),
        DomainRequest::Optimization(_) => Ok(SelectedRepresentation { family: "optimization_problem" }),
    }
}

fn select_polynomial(session: &Session, request: &PolynomialRequest) -> Result<SelectedRepresentation, Diagnostic> {
    for r in refs_from_request(request) {
        let poly = session.polynomial_objects.get(r).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "plan_select")
                .detail("reason", "missing_polynomial_ref")
                .arg("ref", r.0)
        })?;
        if session.rings.get(poly.ring()).is_none() {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "plan_select")
                .detail("reason", "missing_ring_for_polynomial")
                .arg("ring", poly.ring().0));
        }
    }
    Ok(SelectedRepresentation { family: "canonical_distributed_sparse" })
}

fn select_linear_algebra(session: &Session, request: &LinearAlgebraRequest) -> Result<SelectedRepresentation, Diagnostic> {
    let check = |r: crate::domains::linear_algebra::MatrixRef| -> Result<(), Diagnostic> {
        if session.matrix_objects.get(r).is_some() {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "plan_select")
                .detail("reason", "missing_matrix_ref")
                .arg("ref", r.0))
        }
    };
    match *request {
        LinearAlgebraRequest::Transpose { matrix }
        | LinearAlgebraRequest::Index { matrix, .. }
        | LinearAlgebraRequest::Rank { matrix }
        | LinearAlgebraRequest::Det { matrix }
        | LinearAlgebraRequest::Rref { matrix } => check(matrix)?,
        LinearAlgebraRequest::MatMul { lhs, rhs }
        | LinearAlgebraRequest::Hadamard { lhs, rhs }
        | LinearAlgebraRequest::Solve { a: lhs, b: rhs } => {
            check(lhs)?;
            check(rhs)?;
        }
    }
    Ok(SelectedRepresentation { family: "matrix_object_store" })
}
