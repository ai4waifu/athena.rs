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
fn machine_rank_projects_candidate_full_coverage() {
    let mut session = Session::new();
    let domain = DomainResult::LinearAlgebra(LinearAlgebraResult::Ok {
        value: LinearAlgebraValue::MachineRank {
            rank: 2,
            guarantee: AlgorithmGuarantee::Approximate,
        },
    });
    let result = computation_from_domain(&mut session, domain);
    assert_eq!(result.status, ComputationStatus::Candidate);
    assert!(!result.status.is_unconditional_exact());
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
}

#[test]
fn machine_solve_projects_candidate_full_coverage() {
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
    assert_eq!(result.status, ComputationStatus::Candidate);
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Full);
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
