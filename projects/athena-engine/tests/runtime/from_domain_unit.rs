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
fn machine_rank_projects_candidate_not_exact() {
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
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
}

#[test]
fn machine_solve_projects_candidate_partial_coverage() {
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
    assert_eq!(result.coverage, athena_engine::runtime::results::CoverageStatus::Partial);
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
