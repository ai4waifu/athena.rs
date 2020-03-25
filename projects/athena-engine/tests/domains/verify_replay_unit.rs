//! 自 `src/domains/verify_replay.rs` 迁出的原内联测试。

use athena_engine::{
    Session,
    domains::{
        calculus::{CalculusRequest, CalculusResult, CalculusValue, DerivativeOrder},
        group::GroupResult,
        linear_algebra::execute_linear_algebra,
        *,
    },
};
use athena_graph::GraphDirection;
use athena_numeric::Integer;
use athena_types::{AssumptionSet, Diagnostic, DiagnosticCode, SymbolId, TermId};

#[test]
fn calculus_forged_exact_term_fails_recompute() {
    let mut session = Session::new();
    let snapshot = VerifySnapshot::Calculus(CalculusRequest::Derivative {
        expression: TermId(0),
        variable: SymbolId(0),
        order: DerivativeOrder::First,
        assumptions: AssumptionSet::empty(),
    });
    let forged = DomainResult::Calculus(CalculusResult::Exact { value: CalculusValue::Expression(TermId(999_999)), conditions: Vec::new() });
    let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("calculus_recompute_mismatch"));
}

#[test]
fn polynomial_forged_exact_fails_recompute() {
    use athena_engine::domains::polynomial::{
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
    use athena_engine::domains::linear_algebra::{LinearAlgebraRequest, MatrixValue};
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
    let honest = DomainResult::LinearAlgebra(execute_linear_algebra(request.owning_copy(), &session.matrix_objects));
    let snapshot = VerifySnapshot::LinearAlgebra(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn number_theory_gcd_recompute_accepts_honest_claim() {
    use athena_engine::domains::number_theory::{NumberTheoryRequest, execute_number_theory};

    let mut session = Session::new();
    let request = NumberTheoryRequest::Gcd { a: Integer::from_i64(48), b: Integer::from_i64(18) };
    let honest = DomainResult::NumberTheory(execute_number_theory(request.owning_copy()));
    let snapshot = VerifySnapshot::NumberTheory(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn number_theory_forged_gcd_fails_recompute() {
    use athena_engine::domains::number_theory::{NumberTheoryRequest, NumberTheoryResult, NumberTheoryValue};

    let mut session = Session::new();
    let snapshot = VerifySnapshot::NumberTheory(NumberTheoryRequest::Gcd { a: Integer::from_i64(48), b: Integer::from_i64(18) });
    let forged = DomainResult::NumberTheory(NumberTheoryResult::Exact { value: NumberTheoryValue::Integer(Integer::from_i64(7)) });
    let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("number_theory_recompute_mismatch"));
}

#[test]
fn linear_algebra_forged_det_fails_recompute() {
    use athena_engine::domains::linear_algebra::{
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
    use athena_engine::domains::graph_theory::{
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
    let honest = DomainResult::GraphTheory(execute_graph_theory(request.owning_copy()));
    let snapshot = VerifySnapshot::GraphTheory(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn graph_theory_forged_components_fail_recompute() {
    use athena_engine::domains::graph_theory::{
        GraphDomainSemantics, GraphHandle, GraphObject, GraphTheoryRequest, GraphTheoryResult, GraphTheoryValue, WeightDomain,
        execute_graph_theory,
    };

    let mut session = Session::new();
    let graph = GraphObject::from_edges(
        GraphHandle { id: 1, node_count: 2 },
        GraphDomainSemantics::new(GraphDirection::Undirected, WeightDomain::Unweighted),
        vec![(athena_graph::NodeId(0), athena_graph::NodeId(1), 1)],
    );
    let request = GraphTheoryRequest::ConnectedComponents { graph };
    let honest = execute_graph_theory(request.owning_copy());
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
    use athena_engine::domains::optimization::{
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
    let honest = DomainResult::Optimization(execute_optimization(request.owning_copy()));
    let snapshot = VerifySnapshot::Optimization(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn optimization_forged_optimal_fails_recompute() {
    use athena_engine::domains::optimization::{
        AlgorithmPolicy, BoundCertificate, CertificateKind, DecisionVariable, FeasibleSet, Objective, ObjectiveId, ObjectiveSense,
        OptimalityKind, OptimizationLimits, OptimizationProblem, OptimizationRequest, OptimizationResult, ProblemClass, ProblemId, VariableId,
        fingerprint_placeholder,
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

#[test]
fn group_theory_order_recompute_accepts_honest_claim() {
    use athena_engine::domains::group::{GroupRequest, Permutation, execute_group_with_table_mut};

    let mut session = Session::new();
    let gens = vec![Permutation { images: vec![1, 0, 2] }];
    let group = match execute_group_with_table_mut(GroupRequest::PermutationGroup { degree: 3, generators: gens }, &mut session.groups) {
        GroupResult::Exact { value: athena_engine::domains::group::GroupDomainValue::Group(g) } => g.id,
        other => panic!("expected group, got {other:?}"),
    };
    let request = GroupRequest::Order { group };
    let honest = DomainResult::GroupTheory(execute_group_with_table_mut(request.owning_copy(), &mut session.groups));
    let snapshot = VerifySnapshot::GroupTheory(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn group_theory_forged_order_fails_recompute() {
    use athena_engine::domains::group::{GroupDomainValue, GroupRequest, GroupResult, Permutation, execute_group_with_table_mut};

    let mut session = Session::new();
    let gens = vec![Permutation { images: vec![1, 0] }];
    let group = match execute_group_with_table_mut(GroupRequest::PermutationGroup { degree: 2, generators: gens }, &mut session.groups) {
        GroupResult::Exact { value: GroupDomainValue::Group(g) } => g.id,
        other => panic!("expected group, got {other:?}"),
    };
    let snapshot = VerifySnapshot::GroupTheory(GroupRequest::Order { group });
    let forged = DomainResult::GroupTheory(GroupResult::Exact { value: GroupDomainValue::Integer(Integer::from_i64(99)) });
    let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("group_theory_recompute_mismatch"));
}

#[test]
fn field_theory_prime_field_recompute_accepts_honest_claim() {
    use athena_engine::domains::field::{FieldRequest, execute_field_with_table_mut};

    let mut session = Session::new();
    let request = FieldRequest::PrimeField { characteristic: Integer::from_i64(5) };
    let honest = DomainResult::FieldTheory(execute_field_with_table_mut(request.owning_copy(), session.rings.field_table_mut()));
    let snapshot = VerifySnapshot::FieldTheory(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn field_theory_forged_lookup_fails_recompute() {
    use athena_engine::domains::field::{FieldDomainValue, FieldRequest, FieldResult, execute_field_with_table_mut};

    let mut session = Session::new();
    let field = match execute_field_with_table_mut(
        FieldRequest::PrimeField { characteristic: Integer::from_i64(7) },
        session.rings.field_table_mut(),
    ) {
        FieldResult::Exact { value: FieldDomainValue::Field(f) } => f.id,
        other => panic!("expected field, got {other:?}"),
    };
    let snapshot = VerifySnapshot::FieldTheory(FieldRequest::Lookup { field });
    let forged = DomainResult::FieldTheory(FieldResult::Exact { value: FieldDomainValue::Placeholder });
    let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("field_theory_recompute_mismatch"));
}

#[test]
fn galois_theory_is_galois_recompute_accepts_honest_claim() {
    use athena_engine::domains::galois::{GaloisRequest, execute_galois_with_tables};

    let mut session = Session::new();
    // 𝔽₄ = 𝔽₂[x]/(x²+x+1)
    let field = session
        .rings
        .field_table_mut()
        .polynomial_basis_field(Integer::from_i64(2), vec![Integer::from_i64(1), Integer::from_i64(1), Integer::from_i64(1)])
        .expect("F4");
    let extension = session.rings.field_table().extension_by_field(field).expect("ext").id;
    let request = GaloisRequest::IsGalois { extension };
    let honest =
        DomainResult::GaloisTheory(execute_galois_with_tables(request.owning_copy(), session.rings.field_table_mut(), &mut session.groups));
    let snapshot = VerifySnapshot::GaloisTheory(request);
    verify_recompute_domain_result(&mut session, &snapshot, &honest).expect("honest");
}

#[test]
fn galois_theory_forged_is_galois_fails_recompute() {
    use athena_engine::domains::galois::{GaloisDomainValue, GaloisRequest, GaloisResult};

    let mut session = Session::new();
    let field = session
        .rings
        .field_table_mut()
        .polynomial_basis_field(Integer::from_i64(2), vec![Integer::from_i64(1), Integer::from_i64(1), Integer::from_i64(1)])
        .expect("F4");
    let extension = session.rings.field_table().extension_by_field(field).expect("ext").id;
    let snapshot = VerifySnapshot::GaloisTheory(GaloisRequest::IsGalois { extension });
    let forged = DomainResult::GaloisTheory(GaloisResult::Exact { value: GaloisDomainValue::Boolean(false) });
    let err = verify_recompute_domain_result(&mut session, &snapshot, &forged).expect_err("forge");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("galois_theory_recompute_mismatch"));
}
