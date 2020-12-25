//! `DomainResult` → `ComputationResult` 状态投影合同。

use athena_engine::{
    domains::{
        DomainResult,
        linear_algebra::{
            AlgorithmGuarantee, LinearAlgebraResult, LinearAlgebraValue, MachineSolveResult, SolveDisposition,
        },
    },
    runtime::{results::computation_from_domain, session::Session},
};
use athena_types::ComputationStatus;

#[test]
fn machine_rank_projects_approximate_full_coverage() {
    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::MachineRank {
            rank: 2,
            guarantee: AlgorithmGuarantee::Approximate,
        },
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Approximate);
    assert!(!result.status.is_unconditional_exact());
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    let term = result.symbolic_term.expect("machine rank projects int");
    assert!(matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer() == Some(2)
    ));
}

#[test]
fn singular_machine_solve_projects_disposition_and_witness_evidence() {
    use athena_engine::domains::linear_algebra::MachineSolveWitness;
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};

    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::MachineSolve(MachineSolveResult {
            disposition: SolveDisposition::Singular,
            solution: None,
            witness: Some(MachineSolveWitness {
                residual_inf: None,
                numerical_rank: 1,
                pivot_threshold: 1e-12,
            }),
            guarantee: AlgorithmGuarantee::Approximate,
        }),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Partial);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("disposition=Singular")
        )),
        "Singular MachineSolve must publish disposition, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("numerical_rank=1") && summary.contains("pivot_threshold=")
        )),
        "Singular MachineSolve must publish witness evidence, got {:?}",
        result.evidence
    );
    let term = result.symbolic_term.expect("singular projects residual Extension");
    assert!(matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Application { .. })
    ));
}

#[test]
fn machine_solve_projects_approximate_full_coverage() {
    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::MachineSolve(MachineSolveResult {
            disposition: SolveDisposition::Unique,
            solution: None,
            witness: None,
            guarantee: AlgorithmGuarantee::Approximate,
        }),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Approximate);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    assert!(result.symbolic_term.is_some(), "missing solution still gets residual Extension");
}

#[test]
fn exact_rank_still_projects_exact_full() {
    use athena_engine::domains::linear_algebra::ExactRankResult;

    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactRank(ExactRankResult {
            rank: 1,
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    let term = result.symbolic_term.expect("exact rank projects int");
    assert!(matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Atom(athena_ir::Atom::Number(n))) if n.as_exact_integer() == Some(1)
    ));
}

#[test]
fn machine_solve_with_solution_projects_list_term() {
    use athena_engine::domains::linear_algebra::{MatrixResult, MatrixValue};

    let mut session = Session::new();
    let solution = MatrixResult::from_owned(
        MatrixValue::from_f64_row_major(2, 1, vec![1.0, 2.0]).expect("column"),
        AlgorithmGuarantee::Approximate,
    )
    .with_machine_witness(1e-14, Some(1.5));
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::MachineSolve(MachineSolveResult {
            disposition: SolveDisposition::Unique,
            solution: Some(solution),
            witness: None,
            guarantee: AlgorithmGuarantee::Approximate,
        }),
    });
    let before = session.matrix_objects.len();
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Approximate);
    assert!(session.matrix_objects.len() > before, "MachineSolve solution must be interned");
    let term = result.symbolic_term.expect("machine solve projects list");
    assert!(matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Collection { elements, .. }) if elements.len() == 2
    ));
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            athena_engine::runtime::results::ResultEvidence::TrustedKernelSummary { summary, .. }
                if summary.contains("matrix_ref=")
        )),
        "MachineSolve must publish matrix_ref evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            athena_engine::runtime::results::ResultEvidence::TrustedKernelSummary { summary, .. }
                if summary.contains("residual_inf=") && summary.contains("conditioning=")
        )),
        "Unique MachineSolve must publish residual+conditioning evidence, got {:?}",
        result.evidence
    );
}

#[test]
fn matrix_result_envelope_projects_shape_evidence_and_status() {
    use athena_engine::domains::linear_algebra::{MatrixResult, MatrixValue};
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let value = MatrixValue::from_integers_row_major(2, 2, vec![
        Integer::from_i64(1),
        Integer::from_i64(2),
        Integer::from_i64(3),
        Integer::from_i64(4),
    ])
    .expect("matrix");
    let envelope = MatrixResult::from_owned(value, AlgorithmGuarantee::Exact);
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::Matrix(envelope),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    assert!(result.symbolic_term.is_some());
    assert_eq!(session.matrix_objects.len(), 1, "owned Matrix envelope must be interned");
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("shape=2x2") && summary.contains("guarantee=Exact")
        )),
        "expected shape/guarantee evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=") && summary.contains("revision=")
        )),
        "expected matrix_ref/revision evidence, got {:?}",
        result.evidence
    );
}

#[test]
fn dot_matrix_result_envelope_projects_shape_evidence_and_status() {
    use athena_engine::domains::linear_algebra::MatrixValue;
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let value = MatrixValue::from_integers_row_major(1, 1, vec![Integer::from_i64(14)]).expect("scalar-like");
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::dot_outcome(value),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
    assert!(result.symbolic_term.is_some());
    assert_eq!(session.matrix_objects.len(), 1, "owned Dot envelope must be interned");
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("shape=1x1") && summary.contains("guarantee=Exact")
        )),
        "Dot envelope must publish shape/guarantee with the same result, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=") && summary.contains("revision=")
        )),
        "Dot envelope must publish matrix_ref/revision, got {:?}",
        result.evidence
    );
}

#[test]
fn exact_solve_particular_projects_matrix_ref_evidence() {
    use athena_engine::domains::linear_algebra::{ExactSolveResult, MatrixResult, MatrixValue};
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let particular = MatrixResult::from_owned(
        MatrixValue::from_integers_row_major(2, 1, vec![Integer::from_i64(1), Integer::from_i64(2)]).expect("col"),
        AlgorithmGuarantee::Exact,
    );
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactSolve(ExactSolveResult {
            disposition: SolveDisposition::Unique,
            particular: Some(particular),
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let before = session.matrix_objects.len();
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert!(session.matrix_objects.len() > before, "ExactSolve particular must be interned");
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=")
        )),
        "ExactSolve must publish matrix_ref evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("shape=2x1")
        )),
        "ExactSolve must publish shape evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("disposition=Unique")
        )),
        "ExactSolve must publish disposition evidence, got {:?}",
        result.evidence
    );
}

#[test]
fn exact_inverse_singular_projects_disposition_evidence() {
    use athena_engine::domains::linear_algebra::ExactInverseResult;
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};

    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactInverse(ExactInverseResult {
            disposition: SolveDisposition::Singular,
            inverse: None,
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Partial);
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("disposition=Singular")
        )),
        "ExactInverse Singular must publish disposition, got {:?}",
        result.evidence
    );
}

#[test]
fn exact_nullspace_projects_nullity_and_matrix_ref_evidence() {
    use athena_engine::domains::linear_algebra::{ExactNullSpaceResult, MatrixResult, MatrixValue};
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let basis = MatrixValue::from_integers_row_major(1, 2, vec![Integer::from_i64(-2), Integer::from_i64(1)]).expect("basis");
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactNullSpace(ExactNullSpaceResult {
            basis: MatrixResult::from_owned(basis, AlgorithmGuarantee::Exact),
            free_cols: vec![1],
            nullity: 1,
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let before = session.matrix_objects.len();
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert!(session.matrix_objects.len() > before, "ExactNullSpace basis must be interned");
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("nullity=1") && summary.contains("free_cols=")
        )),
        "ExactNullSpace must publish nullity/free_cols, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=") && summary.contains("revision=")
        )),
        "ExactNullSpace must publish matrix_ref/revision, got {:?}",
        result.evidence
    );
}

#[test]
fn exact_rref_envelope_projects_matrix_ref_evidence() {
    use athena_engine::domains::linear_algebra::{ExactRrefResult, MatrixResult, MatrixValue};
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let value = MatrixValue::from_integers_row_major(2, 2, vec![
        Integer::from_i64(1),
        Integer::from_i64(0),
        Integer::from_i64(0),
        Integer::from_i64(1),
    ])
    .expect("identity");
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactRref(ExactRrefResult {
            matrix: MatrixResult::from_owned(value, AlgorithmGuarantee::Exact),
            pivot_cols: vec![0, 1],
            rank: 2,
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let before = session.matrix_objects.len();
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert!(session.matrix_objects.len() > before, "ExactRref matrix must be interned");
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=") && summary.contains("revision=")
        )),
        "ExactRref must publish matrix_ref/revision, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("shape=2x2")
        )),
        "ExactRref must publish shape evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("rank=2") && summary.contains("pivot_cols=2")
        )),
        "ExactRref must publish rank/pivot evidence, got {:?}",
        result.evidence
    );
}

#[test]
fn infinite_exact_solve_projects_free_vars_evidence() {
    use athena_engine::domains::linear_algebra::{ExactSolveResult, MatrixResult, MatrixValue};
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_numeric::Integer;

    let mut session = Session::new();
    let particular = MatrixResult::from_owned(
        MatrixValue::from_integers_row_major(2, 1, vec![Integer::from_i64(2), Integer::from_i64(0)]).expect("col"),
        AlgorithmGuarantee::Exact,
    );
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactSolve(ExactSolveResult {
            disposition: SolveDisposition::Infinite { free_vars: vec![1] },
            particular: Some(particular),
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("disposition=Infinite")
                && summary.contains("free_var_count=1")
                && summary.contains("free_vars=[1]")
        )),
        "Infinite ExactSolve must publish free_vars evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=")
        )),
        "Infinite ExactSolve particular must still intern, got {:?}",
        result.evidence
    );
}

#[test]
fn machine_matrix_envelope_projects_approximate_with_residual_evidence() {
    use athena_engine::domains::linear_algebra::{MatrixResult, MatrixValue};
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};

    let mut session = Session::new();
    let value = MatrixValue::from_f64_row_major(1, 1, vec![1.5]).expect("machine");
    let envelope = MatrixResult::from_owned(value, AlgorithmGuarantee::Approximate).with_machine_witness(1e-12, Some(2.0));
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::Matrix(envelope),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Approximate);
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("residual_inf=") && summary.contains("conditioning=")
        )),
        "expected residual evidence, got {:?}",
        result.evidence
    );
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("matrix_ref=") && summary.contains("revision=")
        )),
        "machine Matrix envelope must publish matrix_ref/revision, got {:?}",
        result.evidence
    );
}

#[test]
fn inconsistent_exact_solve_projects_disposition_residual() {
    use athena_engine::domains::linear_algebra::ExactSolveResult;
    use athena_engine::runtime::results::{ResultEvidence, ResultProviderId};
    use athena_engine::runtime::values::arena::application_display_name;
    use athena_ir::Atom;

    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::ExactSolve(ExactSolveResult {
            disposition: SolveDisposition::Inconsistent,
            particular: None,
            guarantee: AlgorithmGuarantee::Exact,
        }),
    });
    let result = computation_from_domain(&mut session, domain);
    let term = result.symbolic_term.expect("inconsistent projects residual");
    assert_eq!(application_display_name(&session, term).as_deref(), Some("LinearSolve"));
    match session.arena.get(term) {
        Some(athena_ir::TermNode::Application { arguments, .. }) if arguments.len() == 1 => {
            assert!(matches!(session.arena.get(arguments[0]), Some(athena_ir::TermNode::Atom(Atom::Symbol(_)))));
        }
        other => panic!("expected LinearSolve[Inconsistent], got {other:?}"),
    }
    assert!(
        result.evidence.iter().any(|e| matches!(
            e,
            ResultEvidence::TrustedKernelSummary {
                provider: ResultProviderId::LINEAR_ALGEBRA,
                summary,
            } if summary.contains("disposition=Inconsistent")
        )),
        "inconsistent ExactSolve must publish disposition evidence, got {:?}",
        result.evidence
    );
}

#[test]
fn calculus_series_projects_symbolic_polynomial_term() {
    use athena_engine::{
        api::{AthenaRequest, DomainGoal},
        domains::{DomainRequest, calculus::*},
        execution::execute_ir_request,
    };
    use athena_ir::{SemanticOperator, UnaryFunction};
    use athena_types::AssumptionSet;

    let mut session = Session::new();
    let (expression, variable, center) = {
        let dc = athena_engine::domains::DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let expression = dc.apply_semantic(SemanticOperator::Unary(UnaryFunction::Exp), vec![xs]);
        (expression, variable, dc.in_(0))
    };
    let result_id = execute_ir_request(
        &mut session,
        AthenaRequest::Goal(DomainGoal::Dispatch(DomainRequest::Calculus(CalculusRequest::Series {
            expression,
            variable,
            center,
            order: 2,
            assumptions: AssumptionSet::empty(),
        }))),
    )
    .expect("series goal");
    let term = session.results.get(result_id).expect("result").symbolic_term.expect("series bridge term");
    // Expect a Plus tree with three summands (1 + x + (1/2) x^2), not residual Series[…].
    assert!(matches!(
        session.arena.get(term),
        Some(athena_ir::TermNode::Application {
            head: athena_ir::ApplicationHead::Semantic(SemanticOperator::Add),
            arguments,
        }) if arguments.len() == 3
    ));
}

#[test]
fn calculus_exact_projects_candidate_until_admitted() {
    use athena_engine::domains::calculus::{CalculusResult, CalculusValue};

    let mut session = Session::new();
    let term = session.builder().int(0, Default::default());
    let domain = DomainResult::Calculus(CalculusResult::Exact {
        value: CalculusValue::Expression(term),
        conditions: Vec::new(),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Candidate);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
}

#[test]
fn calculus_exact_projects_exact_after_admission_journal() {
    use athena_engine::{
        domains::calculus::{CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder},
        reasoning::mgraph::{AdmissionGate, CalculusRelationKind, VerificationPolicy},
    };
    use athena_ir::SemanticOperator;
    use athena_types::AssumptionSet;

    let mut session = Session::new();
    let (expression, variable) = {
        let dc = athena_engine::domains::DomainExecutionContext::new(&mut session);
        let variable = dc.intern("x");
        let xs = dc.symbol_id(variable);
        let expression = dc.apply_semantic(SemanticOperator::Power, vec![xs, dc.in_(2)]);
        (expression, variable)
    };
    let request = CalculusRequest::Derivative {
        expression,
        variable,
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    };
    let honest = athena_engine::domains::calculus::execute_calculus(&mut session, request.owning_copy());
    let CalculusResult::Exact { value: CalculusValue::Expression(result_term), .. } = honest
    else {
        panic!("expected exact derivative");
    };
    AdmissionGate::admit_calculus_relation(
        &mut session,
        &request,
        CalculusRelationKind::DerivativeOf,
        result_term,
        &VerificationPolicy::default(),
    )
    .expect("admit");
    let domain = DomainResult::Calculus(CalculusResult::Exact {
        value: CalculusValue::Expression(result_term),
        conditions: Vec::new(),
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Exact);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
}

#[test]
fn graph_exact_projects_candidate_not_exact() {
    use athena_engine::domains::graph_theory::{
        GraphDomainSemantics, GraphHandle, GraphObject, GraphTheoryRequest, GraphTheoryResult, WeightDomain, execute_graph_theory,
    };
    use athena_graph::GraphDirection;

    let mut session = Session::new();
    let graph = GraphObject::from_edges(
        GraphHandle { id: 1, node_count: 2 },
        GraphDomainSemantics::new(GraphDirection::Undirected, WeightDomain::Unweighted),
        vec![(athena_graph::NodeId(0), athena_graph::NodeId(1), 1)],
    );
    let honest = execute_graph_theory(GraphTheoryRequest::ConnectedComponents { graph });
    let GraphTheoryResult::Exact { .. } = &honest
    else {
        panic!("expected exact components");
    };
    let result = computation_from_domain(&mut session, DomainResult::GraphTheory(honest));
    assert_eq!(result.status, ComputationStatus::Candidate);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
}

#[test]
fn polynomial_exact_projects_candidate_not_exact() {
    use athena_engine::domains::polynomial::{PolynomialDomainValue, PolynomialResult};

    let mut session = Session::new();
    let domain = DomainResult::Polynomial(PolynomialResult::Exact {
        value: PolynomialDomainValue::Placeholder,
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Candidate);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
}
