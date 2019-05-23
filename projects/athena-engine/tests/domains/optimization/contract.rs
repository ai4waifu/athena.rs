//! 优化合同骨架：变量/可行集/问题/结果状态。

use athena_engine::{
    domains::{
        DomainRequest, DomainResult, execute_domain,
        optimization::{
            AlgorithmPolicy, Constraint as OptConstraint, ConstraintId, ConstraintRelation, DecisionVariable, FeasibleSet,
            Integrality, Objective, ObjectiveId, ObjectiveSense, OptimizationLimits, OptimizationProblem, OptimizationRequest,
            OptimizationResult, ProblemClass, ProblemId, VariableDomain, VariableId, execute_optimization,
        },
    },
    runtime::Session,
};
use athena_types::{AssumptionScopeId, DomainId, TermId};

fn sample_lp() -> OptimizationProblem {
    let variables = vec![DecisionVariable::continuous_real(VariableId(0))];
    let mut feasible = FeasibleSet::empty(DomainId(0));
    feasible.constraints.push(OptConstraint {
        id: ConstraintId(0),
        relation: ConstraintRelation::LessEqual,
        expression: TermId(10),
        domain: DomainId(0),
        provenance: None,
    });
    let objectives =
        vec![Objective { id: ObjectiveId(0), sense: ObjectiveSense::Minimize, expression: TermId(20), priority: 0 }];
    OptimizationProblem::try_new(
        ProblemId(1),
        ProblemClass::LinearProgram,
        variables,
        feasible,
        objectives,
        AssumptionScopeId(0),
        AlgorithmPolicy::default(),
        OptimizationLimits::default(),
    )
    .expect("valid LP skeleton")
}

#[test]
fn rejects_integrality_domain_mismatch() {
    let mut var = DecisionVariable::continuous_real(VariableId(0));
    var.integrality = Integrality::Integer;
    assert!(!var.integrality_consistent());
    assert_eq!(var.domain, VariableDomain::Real);

    let err = OptimizationProblem::try_new(
        ProblemId(2),
        ProblemClass::MixedIntegerLinearProgram,
        vec![var],
        FeasibleSet::empty(DomainId(0)),
        vec![Objective { id: ObjectiveId(0), sense: ObjectiveSense::Minimize, expression: TermId(1), priority: 0 }],
        AssumptionScopeId(0),
        AlgorithmPolicy::default(),
        OptimizationLimits::default(),
    )
    .expect_err("mismatch must fail");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("integrality_domain_mismatch"));
}

#[test]
fn rejects_empty_objectives() {
    let err = OptimizationProblem::try_new(
        ProblemId(3),
        ProblemClass::LinearProgram,
        vec![DecisionVariable::continuous_real(VariableId(0))],
        FeasibleSet::empty(DomainId(0)),
        Vec::new(),
        AssumptionScopeId(0),
        AlgorithmPolicy::default(),
        OptimizationLimits::default(),
    )
    .expect_err("empty objectives must fail");
    assert_eq!(err.details.get("reason").map(|v| v.to_string()).as_deref(), Some("empty_objectives"));
}

#[test]
fn execute_optimization_is_unevaluated_bootstrap() {
    let problem = sample_lp();
    let result = execute_optimization(OptimizationRequest::Solve { problem: problem.clone() });
    match result {
        OptimizationResult::Unevaluated { reason } => {
            assert_eq!(reason.details.get("operation").map(|v| v.to_string()).as_deref(), Some("solve"));
            assert_eq!(reason.details.get("note").map(|v| v.to_string()).as_deref(), Some("bootstrap_contract_only"));
        }
        other => panic!("expected Unevaluated, got {other:?}"),
    }

    let req = DomainRequest::Optimization(OptimizationRequest::ValidateProblem { problem });
    let mut session = Session::new();
    let DomainResult::Optimization(OptimizationResult::Unevaluated { .. }) = execute_domain(&mut session, req).unwrap()
    else {
        panic!("expected optimization Unevaluated via DomainRequest");
    };
}
