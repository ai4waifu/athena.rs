//! PlanIR `Verify` recalculation (Living `28` / `29` bootstrap).
//!
//! Re-runs calculus / polynomial / linear-algebra / number-theory / graph-theory /
//! optimization providers and compares against the claimed `DomainResult`. Other
//! domains keep a typed-presence gate until they gain independent verifiers.
//!
//! **Does not** write AdmissionGate / SemanticCore. Certificate↔proposition
//! matching remains in [`crate::reasoning::mgraph::EvidenceVerifier`].

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, execute_calculus},
        dispatch::{DomainRequest, DomainResult},
        graph_theory::{GraphTheoryRequest, GraphTheoryResult, execute_graph_theory},
        linear_algebra::{LinearAlgebraRequest, LinearAlgebraResult, execute_linear_algebra},
        number_theory::{NumberTheoryRequest, NumberTheoryResult, execute_number_theory},
        optimization::{OptimizationRequest, OptimizationResult, execute_optimization},
        polynomial::{PolynomialRequest, PolynomialResult, execute_polynomial_with_rings},
    },
    runtime::session::Session,
};

/// Request snapshot retained across `CallDomainProvider` for Verify recompute.
#[derive(Debug, Clone)]
pub enum VerifySnapshot {
    /// Clone of a calculus request.
    Calculus(CalculusRequest),
    /// Clone of a polynomial request.
    Polynomial(PolynomialRequest),
    /// Clone of a linear-algebra request.
    LinearAlgebra(LinearAlgebraRequest),
    /// Clone of a number-theory request.
    NumberTheory(NumberTheoryRequest),
    /// Clone of a graph-theory request.
    GraphTheory(GraphTheoryRequest),
    /// Clone of an optimization request.
    Optimization(OptimizationRequest),
    /// Domains without independent recompute yet.
    PresenceOnly,
}

impl VerifySnapshot {
    /// Capture a verify snapshot before the provider consumes the request.
    pub fn from_request(request: &DomainRequest) -> Self {
        match request {
            DomainRequest::Calculus(req) => Self::Calculus(req.clone()),
            DomainRequest::Polynomial(req) => Self::Polynomial(req.clone()),
            DomainRequest::LinearAlgebra(req) => Self::LinearAlgebra(req.clone()),
            DomainRequest::NumberTheory(req) => Self::NumberTheory(req.clone()),
            DomainRequest::GraphTheory(req) => Self::GraphTheory(req.clone()),
            DomainRequest::Optimization(req) => Self::Optimization(req.clone()),
            _ => Self::PresenceOnly,
        }
    }
}

/// Recompute and compare claimed provider output (PlanIR Verify body).
pub fn verify_recompute_domain_result(session: &mut Session, snapshot: &VerifySnapshot, claimed: &DomainResult) -> Result<(), Diagnostic> {
    match snapshot {
        VerifySnapshot::Calculus(req) => {
            let DomainResult::Calculus(claimed_calc) = claimed
            else {
                return Err(verify_err("calculus_result_kind_mismatch"));
            };
            let replay = execute_calculus(session, req.clone());
            assert_calculus_match(session, &replay, claimed_calc)
        }
        VerifySnapshot::Polynomial(req) => {
            let DomainResult::Polynomial(claimed_poly) = claimed
            else {
                return Err(verify_err("polynomial_result_kind_mismatch"));
            };
            // Always recompute via rings path (independent of M-Graph cache admit).
            let replay = execute_polynomial_with_rings(req.clone(), &session.rings, &session.polynomial_objects);
            assert_polynomial_match(session, &replay, claimed_poly)
        }
        VerifySnapshot::LinearAlgebra(req) => {
            let DomainResult::LinearAlgebra(claimed_la) = claimed
            else {
                return Err(verify_err("linear_algebra_result_kind_mismatch"));
            };
            // Recompute against Session matrix store (independent of M-Graph cache admit).
            let replay = execute_linear_algebra(req.clone(), &session.matrix_objects);
            assert_linear_algebra_match(&replay, claimed_la)
        }
        VerifySnapshot::NumberTheory(req) => {
            let DomainResult::NumberTheory(claimed_nt) = claimed
            else {
                return Err(verify_err("number_theory_result_kind_mismatch"));
            };
            let replay = execute_number_theory(req.clone());
            assert_number_theory_match(&replay, claimed_nt)
        }
        VerifySnapshot::GraphTheory(req) => {
            let DomainResult::GraphTheory(claimed_gt) = claimed
            else {
                return Err(verify_err("graph_theory_result_kind_mismatch"));
            };
            let replay = execute_graph_theory(req.clone());
            assert_graph_theory_match(&replay, claimed_gt)
        }
        VerifySnapshot::Optimization(req) => {
            let DomainResult::Optimization(claimed_opt) = claimed
            else {
                return Err(verify_err("optimization_result_kind_mismatch"));
            };
            let replay = execute_optimization(req.clone());
            assert_optimization_match(&replay, claimed_opt)
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

fn verify_err(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "plan_exec").detail("reason", reason)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domains::calculus::DerivativeOrder;
    use athena_types::{AssumptionSet, SymbolId, TermId};

    #[test]
    fn calculus_forged_exact_term_fails_recompute() {
        let mut session = Session::new();
        let snapshot = VerifySnapshot::Calculus(CalculusRequest::Derivative {
            expression: TermId(0),
            variable: SymbolId(0),
            order: DerivativeOrder::First,
            assumptions: AssumptionSet::empty(),
        });
        let forged =
            DomainResult::Calculus(CalculusResult::Exact { value: CalculusValue::Expression(TermId(999_999)), conditions: Vec::new() });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("calculus_recompute_mismatch"));
    }

    #[test]
    fn polynomial_forged_exact_fails_recompute() {
        use crate::domains::polynomial::{
            CoefficientDomain, MonomialOrder, PolynomialBuilder, PolynomialDomainValue, PolynomialRequest, PolynomialResult,
        };

        let mut session = Session::new();
        let ring = session.rings.intern(CoefficientDomain::Integer, vec![SymbolId(0)], MonomialOrder::Lex).expect("ring");
        let poly = PolynomialBuilder::new(ring).build(&session.rings).expect("zero");
        let poly_ref = session.polynomial_objects.intern(poly, &session.rings);
        let snapshot = VerifySnapshot::Polynomial(PolynomialRequest::Normalize { polynomial: poly_ref });
        let forged = DomainResult::Polynomial(PolynomialResult::Exact { value: PolynomialDomainValue::Placeholder });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("polynomial_recompute_mismatch"));
    }

    #[test]
    fn linear_algebra_det_recompute_accepts_honest_claim() {
        use crate::domains::linear_algebra::{LinearAlgebraRequest, MatrixValue};
        use athena_numeric::Integer;

        let mut session = Session::new();
        let matrix = MatrixValue::from_integers_row_major(
            2,
            2,
            vec![Integer::from_i64(1), Integer::from_i64(2), Integer::from_i64(3), Integer::from_i64(4)],
        )
        .expect("matrix");
        let matrix_ref = session.matrix_objects.intern(matrix);
        let request = LinearAlgebraRequest::Det { matrix: matrix_ref };
        let honest = DomainResult::LinearAlgebra(execute_linear_algebra(request.clone(), &session.matrix_objects));
        let snapshot = VerifySnapshot::LinearAlgebra(request);
        verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
    }

    #[test]
    fn number_theory_gcd_recompute_accepts_honest_claim() {
        use crate::domains::number_theory::{NumberTheoryRequest, execute_number_theory};
        use athena_numeric::Integer;

        let mut session = Session::new();
        let request = NumberTheoryRequest::Gcd { a: Integer::from_i64(48), b: Integer::from_i64(18) };
        let honest = DomainResult::NumberTheory(execute_number_theory(request.clone()));
        let snapshot = VerifySnapshot::NumberTheory(request);
        verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
    }

    #[test]
    fn number_theory_forged_gcd_fails_recompute() {
        use crate::domains::number_theory::{NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue};
        use athena_numeric::Integer;

        let mut session = Session::new();
        let snapshot = VerifySnapshot::NumberTheory(NumberTheoryRequest::Gcd { a: Integer::from_i64(48), b: Integer::from_i64(18) });
        let forged = DomainResult::NumberTheory(NumberTheoryResult::Exact { value: NumberTheoryValue::Integer(Integer::from_i64(7)) });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("number_theory_recompute_mismatch"));
    }

    #[test]
    fn linear_algebra_forged_det_fails_recompute() {
        use crate::domains::linear_algebra::{
            AlgorithmGuarantee, ExactDetResult, LinearAlgebraRequest, LinearAlgebraResult, LinearAlgebraValue, MatrixValue,
        };
        use athena_numeric::{Integer, Rational};

        let mut session = Session::new();
        let matrix = MatrixValue::from_integers_row_major(
            2,
            2,
            vec![Integer::from_i64(1), Integer::from_i64(0), Integer::from_i64(0), Integer::from_i64(1)],
        )
        .expect("matrix");
        let matrix_ref = session.matrix_objects.intern(matrix);
        let snapshot = VerifySnapshot::LinearAlgebra(LinearAlgebraRequest::Det { matrix: matrix_ref });
        let forged = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
            value: LinearAlgebraValue::ExactDet(ExactDetResult {
                det: Rational::from_integer(Integer::from_i64(99)),
                guarantee: AlgorithmGuarantee::Exact,
            }),
        });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("linear_algebra_recompute_mismatch"));
    }

    #[test]
    fn graph_theory_components_recompute_accepts_honest_claim() {
        use crate::domains::graph_theory::{
            GraphDomainSemantics, GraphHandle, GraphObject, GraphTheoryRequest, WeightDomain, execute_graph_theory,
        };
        use athena_graph::GraphDirection;

        let mut session = Session::new();
        let graph = GraphObject::from_edges(
            GraphHandle { id: 1, node_count: 3 },
            GraphDomainSemantics::new(GraphDirection::Undirected, WeightDomain::Unweighted),
            vec![(athena_graph::NodeId(0), athena_graph::NodeId(1), 1), (athena_graph::NodeId(1), athena_graph::NodeId(2), 1)],
        );
        let request = GraphTheoryRequest::ConnectedComponents { graph };
        let honest = DomainResult::GraphTheory(execute_graph_theory(request.clone()));
        let snapshot = VerifySnapshot::GraphTheory(request);
        verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
    }

    #[test]
    fn graph_theory_forged_components_fail_recompute() {
        use crate::domains::graph_theory::{
            GraphDomainSemantics, GraphHandle, GraphObject, GraphTheoryRequest, GraphTheoryResult, GraphTheoryValue, WeightDomain,
            execute_graph_theory,
        };
        use athena_graph::GraphDirection;

        let mut session = Session::new();
        let graph = GraphObject::from_edges(
            GraphHandle { id: 1, node_count: 2 },
            GraphDomainSemantics::new(GraphDirection::Undirected, WeightDomain::Unweighted),
            vec![(athena_graph::NodeId(0), athena_graph::NodeId(1), 1)],
        );
        let request = GraphTheoryRequest::ConnectedComponents { graph };
        let honest = execute_graph_theory(request.clone());
        let GraphTheoryResult::Exact { value: GraphTheoryValue::ConnectedComponents(mut forged_cc) } = honest
        else {
            panic!("expected connected components");
        };
        forged_cc.component_count = forged_cc.component_count.saturating_add(99);
        let snapshot = VerifySnapshot::GraphTheory(request);
        let forged = DomainResult::GraphTheory(GraphTheoryResult::Exact { value: GraphTheoryValue::ConnectedComponents(forged_cc) });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("graph_theory_recompute_mismatch"));
    }

    #[test]
    fn optimization_validate_recompute_accepts_honest_claim() {
        use crate::domains::optimization::{
            AlgorithmPolicy, DecisionVariable, FeasibleSet, Objective, ObjectiveId, ObjectiveSense, OptimizationLimits, OptimizationProblem,
            OptimizationRequest, ProblemClass, ProblemId, VariableId, execute_optimization,
        };
        use athena_types::{AssumptionScopeId, DomainId, TermId};

        let mut session = Session::new();
        let problem = OptimizationProblem::try_new(
            ProblemId(1),
            ProblemClass::LinearProgram,
            vec![DecisionVariable::continuous_real(VariableId(0))],
            FeasibleSet::empty(DomainId(0)),
            vec![Objective { id: ObjectiveId(0), sense: ObjectiveSense::Minimize, expression: TermId(1), priority: 0 }],
            AssumptionScopeId(0),
            AlgorithmPolicy::default(),
            OptimizationLimits::default(),
        )
        .expect("lp");
        let request = OptimizationRequest::ValidateProblem { problem };
        let honest = DomainResult::Optimization(execute_optimization(request.clone()));
        let snapshot = VerifySnapshot::Optimization(request);
        verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
    }

    #[test]
    fn optimization_forged_optimal_fails_recompute() {
        use crate::domains::optimization::{
            AlgorithmPolicy, BoundCertificate, CertificateKind, DecisionVariable, FeasibleSet, Objective, ObjectiveId, ObjectiveSense,
            OptimalityKind, OptimizationLimits, OptimizationProblem, OptimizationRequest, OptimizationResult, ProblemClass, ProblemId,
            VariableId, fingerprint_placeholder,
        };
        use athena_types::{AssumptionScopeId, ComputationStatus, DomainId, TermId};

        let mut session = Session::new();
        let problem = OptimizationProblem::try_new(
            ProblemId(1),
            ProblemClass::LinearProgram,
            vec![DecisionVariable::continuous_real(VariableId(0))],
            FeasibleSet::empty(DomainId(0)),
            vec![Objective { id: ObjectiveId(0), sense: ObjectiveSense::Minimize, expression: TermId(1), priority: 0 }],
            AssumptionScopeId(0),
            AlgorithmPolicy::default(),
            OptimizationLimits::default(),
        )
        .expect("lp");
        let snapshot = VerifySnapshot::Optimization(OptimizationRequest::ValidateProblem { problem });
        let forged = DomainResult::Optimization(OptimizationResult::Optimal {
            fingerprint: fingerprint_placeholder(1),
            status: ComputationStatus::Verified,
            point: vec![0.0],
            value: 0.0,
            certificate: BoundCertificate {
                kind: CertificateKind::Placeholder,
                optimality: Some(OptimalityKind::Global),
                lower_bound: Some(0.0),
                upper_bound: Some(0.0),
                relative_gap: None,
                proof: None,
                summary: "forged".into(),
            },
        });
        let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
        assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("optimization_recompute_mismatch"));
    }
}
