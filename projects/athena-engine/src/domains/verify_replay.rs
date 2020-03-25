//! `DomainPlan` 的 `Verify` 重算（引导期）。
//!
//! 重跑微积分 / 多项式 / 线性代数 / 数论 / 图论 /
//! 优化 / 群论 / 域论 / 伽罗瓦理论提供者，并与声称的 `DomainResult` 比较。
//! 尚无独立校验器的领域保留类型存在性门闩。
//!
//! **不**写入 `AdmissionGate` / `SemanticCore`。证书↔命题
//! 匹配仍在 [`crate::reasoning::mgraph::EvidenceVerifier`]。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, execute_calculus},
        dispatch::{DomainRequest, DomainResult},
        field::{FieldRequest, FieldResult, execute_field_with_table_mut},
        galois::{GaloisRequest, GaloisResult, execute_galois_with_tables},
        graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
        group::{GroupRequest, GroupResult, execute_group_with_table_mut},
        linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
        number_theory::{NumberTheoryRequest, NumberTheoryResult, execute_number_theory},
        optimization::{OptimizationRequest, OptimizationResult, execute_optimization},
        polynomial::{PolynomialRequest, PolynomialResult, execute_polynomial_with_rings},
    },
    runtime::session::Session,
};

/// 跨越 `CallDomainProvider` 保留、供 `Verify` 重算的请求快照。
///
/// **不**实现 [`Clone`]（`GaloisTheory` / group / field 含 owning 数学载荷）。
#[derive(Debug)]
pub enum VerifySnapshot {
    /// 微积分请求的拥有式深复制。
    Calculus(CalculusRequest),
    /// 多项式请求的拥有式深复制。
    Polynomial(PolynomialRequest),
    /// 线性代数请求的拥有式深复制。
    LinearAlgebra(LinearAlgebraRequest),
    /// 数论请求的拥有式深复制。
    NumberTheory(NumberTheoryRequest),
    /// 图论请求的拥有式深复制。
    GraphTheory(GraphTheoryRequest),
    /// 优化请求的拥有式深复制。
    Optimization(OptimizationRequest),
    /// 群论请求的拥有式深复制（GC `owning_copy`）。
    GroupTheory(GroupRequest),
    /// 域论请求的拥有式深复制（GC `owning_copy`）。
    FieldTheory(FieldRequest),
    /// 伽罗瓦理论请求的拥有式深复制（`Polynomial` 经 GC `owning_copy`）。
    GaloisTheory(GaloisRequest),
    /// 尚无独立重算的领域。
    PresenceOnly,
}

impl VerifySnapshot {
    /// 在提供者消费请求之前捕获校验快照。
    pub fn from_request(request: &DomainRequest) -> Self {
        match request {
            DomainRequest::Calculus(req) => Self::Calculus(req.owning_copy()),
            DomainRequest::Polynomial(req) => Self::Polynomial(req.owning_copy()),
            DomainRequest::LinearAlgebra(req) => Self::LinearAlgebra(req.owning_copy()),
            DomainRequest::NumberTheory(req) => Self::NumberTheory(req.owning_copy()),
            DomainRequest::GraphTheory(req) => Self::GraphTheory(req.owning_copy()),
            DomainRequest::Optimization(req) => Self::Optimization(req.owning_copy()),
            DomainRequest::GroupTheory(req) => Self::GroupTheory(req.owning_copy()),
            DomainRequest::FieldTheory(req) => Self::FieldTheory(req.owning_copy()),
            DomainRequest::GaloisTheory(req) => Self::GaloisTheory(req.owning_copy()),
            _ => Self::PresenceOnly,
        }
    }
}

/// 重算并比对声称的提供者输出（`DomainPlan` 的 `Verify` 体）。
pub fn verify_recompute_domain_result(session: &mut Session, snapshot: &VerifySnapshot, claimed: &DomainResult) -> Result<(), Diagnostic> {
    match snapshot {
        VerifySnapshot::Calculus(req) => {
            let DomainResult::Calculus(claimed_calc) = claimed
            else {
                return Err(verify_err("calculus_result_kind_mismatch"));
            };
            let replay = execute_calculus(session, req.owning_copy());
            assert_calculus_match(session, &replay, claimed_calc)
        }
        VerifySnapshot::Polynomial(req) => {
            let DomainResult::Polynomial(claimed_poly) = claimed
            else {
                return Err(verify_err("polynomial_result_kind_mismatch"));
            };
            // 始终经 rings 路径重算（独立于 M-Graph 缓存接纳）。
            let replay = execute_polynomial_with_rings(req.owning_copy(), &session.rings, &session.polynomial_objects);
            assert_polynomial_match(session, &replay, claimed_poly)
        }
        VerifySnapshot::LinearAlgebra(req) => {
            let DomainResult::LinearAlgebra(claimed_la) = claimed
            else {
                return Err(verify_err("linear_algebra_result_kind_mismatch"));
            };
            // 相对 Session 矩阵存储重算（独立于 M-Graph 缓存接纳）。
            let replay = execute_linear_algebra(req.owning_copy(), &session.matrix_objects);
            assert_linear_algebra_match(&replay, claimed_la)
        }
        VerifySnapshot::NumberTheory(req) => {
            let DomainResult::NumberTheory(claimed_nt) = claimed
            else {
                return Err(verify_err("number_theory_result_kind_mismatch"));
            };
            let replay = execute_number_theory(req.owning_copy());
            assert_number_theory_match(&replay, claimed_nt)
        }
        VerifySnapshot::GraphTheory(req) => {
            let DomainResult::GraphTheory(claimed_gt) = claimed
            else {
                return Err(verify_err("graph_theory_result_kind_mismatch"));
            };
            let replay = execute_graph_theory(req.owning_copy());
            assert_graph_theory_match(&replay, claimed_gt)
        }
        VerifySnapshot::Optimization(req) => {
            let DomainResult::Optimization(claimed_opt) = claimed
            else {
                return Err(verify_err("optimization_result_kind_mismatch"));
            };
            let replay = execute_optimization(req.owning_copy());
            assert_optimization_match(&replay, claimed_opt)
        }
        VerifySnapshot::GroupTheory(req) => {
            let DomainResult::GroupTheory(claimed_group) = claimed
            else {
                return Err(verify_err("group_theory_result_kind_mismatch"));
            };
            // 相对 Session 的 `groups` 表重算（独立于 M-Graph 缓存接纳）。
            let replay = execute_group_with_table_mut(req.owning_copy(), &mut session.groups);
            assert_group_theory_match(&replay, claimed_group)
        }
        VerifySnapshot::FieldTheory(req) => {
            let DomainResult::FieldTheory(claimed_field) = claimed
            else {
                return Err(verify_err("field_theory_result_kind_mismatch"));
            };
            // 相对 Session 域表重算（独立于 M-Graph 缓存接纳）。
            let replay = execute_field_with_table_mut(req.owning_copy(), session.rings.field_table_mut());
            assert_field_theory_match(&replay, claimed_field)
        }
        VerifySnapshot::GaloisTheory(req) => {
            let DomainResult::GaloisTheory(claimed_galois) = claimed
            else {
                return Err(verify_err("galois_theory_result_kind_mismatch"));
            };
            let replay = execute_galois_with_tables(req.owning_copy(), session.rings.field_table_mut(), &mut session.groups);
            assert_galois_theory_match(&replay, claimed_galois)
        }
        VerifySnapshot::PresenceOnly => match claimed {
            DomainResult::Calculus(_)
            | DomainResult::NumberTheory(_)
            | DomainResult::Polynomial(_)
            | DomainResult::GroupTheory(_)
            | DomainResult::FieldTheory(_)
            | DomainResult::GaloisTheory(_)
            | DomainResult::GraphTheory(_)
            | DomainResult::LinearAlgebra(_)
            | DomainResult::Optimization(_) => Ok(()),
        },
    }
}

fn assert_calculus_match(
    session: &Session,
    replay: &CalculusResult<CalculusValue>,
    claimed: &CalculusResult<CalculusValue>,
) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (CalculusResult::Exact { value: rv, conditions: rc }, CalculusResult::Exact { value: cv, conditions: cc }) => {
            if rc != cc {
                return Err(verify_err("calculus_conditions_mismatch"));
            }
            if !calculus_values_match(session, rv, cv) {
                return Err(verify_err("calculus_recompute_mismatch"));
            }
            Ok(())
        }
        (CalculusResult::Unevaluated { .. }, CalculusResult::Unevaluated { .. }) => Ok(()),
        (CalculusResult::Conditional { value: rv, conditions: rc }, CalculusResult::Conditional { value: cv, conditions: cc }) => {
            if rc != cc {
                return Err(verify_err("calculus_conditions_mismatch"));
            }
            if !calculus_values_match(session, rv, cv) {
                return Err(verify_err("calculus_recompute_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("calculus_result_shape_mismatch")),
    }
}

fn calculus_values_match(session: &Session, a: &CalculusValue, b: &CalculusValue) -> bool {
    match (a, b) {
        (CalculusValue::Expression(x), CalculusValue::Expression(y)) => session.arena.structural_eq(*x, *y),
        (CalculusValue::Series(x), CalculusValue::Series(y)) => {
            if x == y {
                return true;
            }
            match (session.series_objects.get(*x), session.series_objects.get(*y)) {
                (Some(sx), Some(sy)) => sx == sy,
                _ => false,
            }
        }
        _ => a == b,
    }
}

fn assert_polynomial_match(session: &Session, replay: &PolynomialResult, claimed: &PolynomialResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (PolynomialResult::Exact { value: rv }, PolynomialResult::Exact { value: cv }) => {
            if rv != cv {
                return Err(verify_err("polynomial_recompute_mismatch"));
            }
            verify_claimed_groebner_basis(session, cv)
        }
        (PolynomialResult::Unevaluated { .. }, PolynomialResult::Unevaluated { .. }) => Ok(()),
        _ => Err(verify_err("polynomial_result_shape_mismatch")),
    }
}

fn verify_claimed_groebner_basis(session: &Session, value: &crate::domains::polynomial::PolynomialDomainValue) -> Result<(), Diagnostic> {
    use crate::domains::polynomial::{PolynomialDomainValue, verify_groebner_basis};
    let PolynomialDomainValue::GroebnerBasis(v) = value
    else {
        return Ok(());
    };
    if !v.is_exact_witness() {
        return Ok(());
    }
    let report = verify_groebner_basis(&v.basis, &session.rings).map_err(|_| verify_err("groebner_independent_verify_failed"))?;
    if report.all_s_pairs_reduce_to_zero { Ok(()) } else { Err(verify_err("groebner_basis_not_complete")) }
}

fn assert_linear_algebra_match(replay: &LinearAlgebraResult, claimed: &LinearAlgebraResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (LinearAlgebraResult::Ok { value: rv }, LinearAlgebraResult::Ok { value: cv }) => {
            if rv != cv {
                return Err(verify_err("linear_algebra_recompute_mismatch"));
            }
            Ok(())
        }
        (LinearAlgebraResult::Err { diagnostic: rd }, LinearAlgebraResult::Err { diagnostic: cd }) => {
            let rr = rd.details.get("reason").map(|v| v.to_string());
            let cr = cd.details.get("reason").map(|v| v.to_string());
            if rr != cr {
                return Err(verify_err("linear_algebra_error_reason_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("linear_algebra_result_shape_mismatch")),
    }
}

fn assert_number_theory_match(replay: &NumberTheoryResult, claimed: &NumberTheoryResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (NumberTheoryResult::Exact { value: rv }, NumberTheoryResult::Exact { value: cv })
        | (NumberTheoryResult::Probable { value: rv }, NumberTheoryResult::Probable { value: cv })
        | (NumberTheoryResult::Partial { value: rv }, NumberTheoryResult::Partial { value: cv })
        | (NumberTheoryResult::ResourceLimited { value: rv }, NumberTheoryResult::ResourceLimited { value: cv })
        | (NumberTheoryResult::Inconclusive { value: rv }, NumberTheoryResult::Inconclusive { value: cv }) => {
            if rv != cv {
                return Err(verify_err("number_theory_recompute_mismatch"));
            }
            Ok(())
        }
        (NumberTheoryResult::InvalidInput { reason: rr }, NumberTheoryResult::InvalidInput { reason: cr })
        | (NumberTheoryResult::Unevaluated { reason: rr }, NumberTheoryResult::Unevaluated { reason: cr }) => {
            let rrs = rr.details.get("reason").map(|v| v.to_string());
            let crs = cr.details.get("reason").map(|v| v.to_string());
            if rrs != crs {
                return Err(verify_err("number_theory_error_reason_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("number_theory_result_shape_mismatch")),
    }
}

fn assert_graph_theory_match(replay: &GraphTheoryResult, claimed: &GraphTheoryResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (GraphTheoryResult::Exact { value: rv }, GraphTheoryResult::Exact { value: cv }) => {
            if rv != cv {
                return Err(verify_err("graph_theory_recompute_mismatch"));
            }
            Ok(())
        }
        (GraphTheoryResult::Unevaluated { reason: rr }, GraphTheoryResult::Unevaluated { reason: cr }) => {
            let rrs = rr.details.get("reason").map(|v| v.to_string());
            let crs = cr.details.get("reason").map(|v| v.to_string());
            if rrs != crs {
                return Err(verify_err("graph_theory_error_reason_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("graph_theory_result_shape_mismatch")),
    }
}

fn assert_optimization_match(replay: &OptimizationResult, claimed: &OptimizationResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (OptimizationResult::Unevaluated { reason: rr }, OptimizationResult::Unevaluated { reason: cr })
        | (OptimizationResult::InvalidInput { reason: rr }, OptimizationResult::InvalidInput { reason: cr }) => {
            let rrs = rr.details.get("reason").map(|v| v.to_string());
            let crs = cr.details.get("reason").map(|v| v.to_string());
            let rop = rr.details.get("operation").map(|v| v.to_string());
            let cop = cr.details.get("operation").map(|v| v.to_string());
            if rrs != crs || rop != cop {
                return Err(verify_err("optimization_error_reason_mismatch"));
            }
            Ok(())
        }
        (a, b) if a == b => Ok(()),
        _ => Err(verify_err("optimization_recompute_mismatch")),
    }
}

fn assert_group_theory_match(replay: &GroupResult, claimed: &GroupResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (GroupResult::Exact { value: rv }, GroupResult::Exact { value: cv }) => {
            if rv != cv {
                return Err(verify_err("group_theory_recompute_mismatch"));
            }
            Ok(())
        }
        (GroupResult::Unevaluated { reason: rr }, GroupResult::Unevaluated { reason: cr }) => {
            let rrs = rr.details.get("reason").map(|v| v.to_string());
            let crs = cr.details.get("reason").map(|v| v.to_string());
            let rop = rr.details.get("operation").map(|v| v.to_string());
            let cop = cr.details.get("operation").map(|v| v.to_string());
            if rrs != crs || rop != cop {
                return Err(verify_err("group_theory_error_reason_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("group_theory_result_shape_mismatch")),
    }
}

fn assert_field_theory_match(replay: &FieldResult, claimed: &FieldResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (FieldResult::Exact { value: rv }, FieldResult::Exact { value: cv }) => {
            if rv != cv {
                return Err(verify_err("field_theory_recompute_mismatch"));
            }
            Ok(())
        }
        (FieldResult::Unevaluated { reason: rr }, FieldResult::Unevaluated { reason: cr }) => {
            let rrs = rr.details.get("reason").map(|v| v.to_string());
            let crs = cr.details.get("reason").map(|v| v.to_string());
            let rop = rr.details.get("operation").map(|v| v.to_string());
            let cop = cr.details.get("operation").map(|v| v.to_string());
            if rrs != crs || rop != cop {
                return Err(verify_err("field_theory_error_reason_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("field_theory_result_shape_mismatch")),
    }
}

fn assert_galois_theory_match(replay: &GaloisResult, claimed: &GaloisResult) -> Result<(), Diagnostic> {
    match (replay, claimed) {
        (GaloisResult::Exact { value: rv }, GaloisResult::Exact { value: cv }) => {
            if rv != cv {
                return Err(verify_err("galois_theory_recompute_mismatch"));
            }
            Ok(())
        }
        (GaloisResult::Unevaluated { reason: rr }, GaloisResult::Unevaluated { reason: cr }) => {
            let rrs = rr.details.get("reason").map(|v| v.to_string());
            let crs = cr.details.get("reason").map(|v| v.to_string());
            let rop = rr.details.get("operation").map(|v| v.to_string());
            let cop = cr.details.get("operation").map(|v| v.to_string());
            if rrs != crs || rop != cop {
                return Err(verify_err("galois_theory_error_reason_mismatch"));
            }
            Ok(())
        }
        _ => Err(verify_err("galois_theory_result_shape_mismatch")),
    }
}

fn verify_err(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "plan_exec").detail("reason", reason)
}
