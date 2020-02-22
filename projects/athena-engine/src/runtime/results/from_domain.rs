//! [`DomainResult`] → [`ComputationResult`] 投影（禁止丢弃领域载荷）。

use athena_types::{ComputationStatus, Condition, Diagnostic, TermId};

use crate::{
    domains::{
        calculus::{CalculusResult, CalculusValue},
        dispatch::DomainResult,
        field::FieldResult,
        galois::GaloisResult,
        graph_theory::GraphTheoryResult,
        group::GroupResult,
        linear_algebra::LinearAlgebraResult,
        number_theory::NumberTheoryResult,
        optimization::OptimizationResult,
        polynomial::PolynomialResult,
    },
    runtime::{
        results::{
            ComputationResult, CoverageStatus, ResultEvidence, ResultProvenance, ResultProviderId, ResultProviderStamp,
        },
        session::Session,
        values::RuntimeValue,
    },
};

/// 将域结果写入 `ValueStore` / `ComputationResult`（保留完整 `DomainResult`）。
pub fn computation_from_domain(session: &mut Session, domain: DomainResult) -> ComputationResult {
    let mapped = map_domain_meta(&domain);
    let value_id = session.insert_value(RuntimeValue::Domain(domain));
    let mut result = ComputationResult::with_status(mapped.status, mapped.coverage)
        .with_value(value_id)
        .with_provenance(ResultProvenance::kind("DomainGoal"));
    if let Some(stamp) = mapped.provider {
        result = result.with_provider_stamp(stamp);
    }
    if let Some(term) = mapped.symbolic_term {
        result = result.with_symbolic_term(term);
    }
    for condition in mapped.conditions {
        result = result.with_condition(condition);
    }
    for diagnostic in mapped.diagnostics {
        result = result.with_diagnostic(diagnostic);
    }
    for evidence in mapped.evidence {
        result = result.with_evidence(evidence);
    }
    result
}

struct DomainMeta {
    status: ComputationStatus,
    coverage: CoverageStatus,
    symbolic_term: Option<TermId>,
    conditions: Vec<Condition>,
    diagnostics: Vec<Diagnostic>,
    evidence: Vec<ResultEvidence>,
    provider: Option<ResultProviderStamp>,
}

fn map_domain_meta(domain: &DomainResult) -> DomainMeta {
    match domain {
        DomainResult::Calculus(r) => map_calculus(r),
        DomainResult::NumberTheory(r) => map_number_theory(r),
        DomainResult::Polynomial(r) => map_polynomial(r),
        DomainResult::GroupTheory(r) => map_group(r),
        DomainResult::FieldTheory(r) => map_field(r),
        DomainResult::GaloisTheory(r) => map_galois(r),
        DomainResult::GraphTheory(r) => map_graph(r),
        DomainResult::LinearAlgebra(r) => map_linear_algebra(r),
        DomainResult::Optimization(r) => map_optimization(r),
    }
}

fn map_calculus(result: &CalculusResult<CalculusValue>) -> DomainMeta {
    match result {
        CalculusResult::Exact { value, conditions } => DomainMeta {
            status: ComputationStatus::Exact,
            coverage: CoverageStatus::Full,
            symbolic_term: calculus_term(value),
            conditions: conditions.clone(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::CALCULUS.stamped()),
        },
        CalculusResult::Conditional { value, conditions } => DomainMeta {
            status: ComputationStatus::Conditional,
            coverage: CoverageStatus::Partial,
            symbolic_term: calculus_term(value),
            conditions: conditions.clone(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::CALCULUS.stamped()),
        },
        CalculusResult::Unevaluated { expression, reason } => DomainMeta {
            status: ComputationStatus::Unknown,
            coverage: CoverageStatus::Unsupported,
            symbolic_term: calculus_term(expression),
            conditions: Vec::new(),
            diagnostics: vec![reason.clone()],
            evidence: Vec::new(),
            provider: Some(ResultProviderId::CALCULUS.stamped()),
        },
    }
}

fn calculus_term(value: &CalculusValue) -> Option<TermId> {
    match value {
        CalculusValue::Expression(term) => Some(*term),
        _ => None,
    }
}

fn map_number_theory(result: &NumberTheoryResult) -> DomainMeta {
    match result {
        NumberTheoryResult::Exact { .. } => exact_provider(ResultProviderId::NUMBER_THEORY),
        NumberTheoryResult::Probable { .. } => DomainMeta {
            status: ComputationStatus::Probable,
            coverage: CoverageStatus::Partial,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::NUMBER_THEORY.stamped()),
        },
        NumberTheoryResult::Partial { .. } => DomainMeta {
            status: ComputationStatus::Partial,
            coverage: CoverageStatus::Partial,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::NUMBER_THEORY.stamped()),
        },
        NumberTheoryResult::ResourceLimited { .. } => DomainMeta {
            status: ComputationStatus::ResourceLimited,
            coverage: CoverageStatus::Partial,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::NUMBER_THEORY.stamped()),
        },
        NumberTheoryResult::Inconclusive { .. } => DomainMeta {
            status: ComputationStatus::Unknown,
            coverage: CoverageStatus::Unknown,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::NUMBER_THEORY.stamped()),
        },
        NumberTheoryResult::InvalidInput { reason } => DomainMeta {
            status: ComputationStatus::Invalid,
            coverage: CoverageStatus::Unsupported,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: vec![reason.clone()],
            evidence: Vec::new(),
            provider: Some(ResultProviderId::NUMBER_THEORY.stamped()),
        },
        NumberTheoryResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::NUMBER_THEORY),
    }
}

fn map_polynomial(result: &PolynomialResult) -> DomainMeta {
    match result {
        PolynomialResult::Exact { .. } => exact_provider(ResultProviderId::POLYNOMIAL),
        PolynomialResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::POLYNOMIAL),
    }
}

fn map_group(result: &GroupResult) -> DomainMeta {
    match result {
        GroupResult::Exact { .. } => exact_provider(ResultProviderId::GROUP),
        GroupResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::GROUP),
    }
}

fn map_field(result: &FieldResult) -> DomainMeta {
    match result {
        FieldResult::Exact { .. } => exact_provider(ResultProviderId::FIELD),
        FieldResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::FIELD),
    }
}

fn map_galois(result: &GaloisResult) -> DomainMeta {
    match result {
        GaloisResult::Exact { .. } => exact_provider(ResultProviderId::GALOIS),
        GaloisResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::GALOIS),
    }
}

fn map_graph(result: &GraphTheoryResult) -> DomainMeta {
    match result {
        GraphTheoryResult::Exact { .. } => exact_provider(ResultProviderId::GRAPH_THEORY),
        GraphTheoryResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::GRAPH_THEORY),
    }
}

fn map_linear_algebra(result: &LinearAlgebraResult) -> DomainMeta {
    match result {
        LinearAlgebraResult::Ok { .. } => exact_provider(ResultProviderId::LINEAR_ALGEBRA),
        LinearAlgebraResult::Err { diagnostic } => DomainMeta {
            status: ComputationStatus::Invalid,
            coverage: CoverageStatus::Unsupported,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: vec![diagnostic.clone()],
            evidence: Vec::new(),
            provider: Some(ResultProviderId::LINEAR_ALGEBRA.stamped()),
        },
    }
}

fn map_optimization(result: &OptimizationResult) -> DomainMeta {
    match result {
        OptimizationResult::Optimal { status, .. }
        | OptimizationResult::Feasible { status, .. }
        | OptimizationResult::Infeasible { status, .. }
        | OptimizationResult::Unbounded { status, .. }
        | OptimizationResult::Inconclusive { status, .. }
        | OptimizationResult::ResourceLimited { status, .. }
        | OptimizationResult::NumericalCandidate { status, .. } => {
            let coverage = match status {
                ComputationStatus::Exact | ComputationStatus::Verified => CoverageStatus::Full,
                ComputationStatus::Partial
                | ComputationStatus::Conditional
                | ComputationStatus::Probable
                | ComputationStatus::Candidate
                | ComputationStatus::ResourceLimited => CoverageStatus::Partial,
                ComputationStatus::Unknown | ComputationStatus::Invalid => CoverageStatus::Unknown,
            };
            DomainMeta {
                status: *status,
                coverage,
                symbolic_term: None,
                conditions: Vec::new(),
                diagnostics: Vec::new(),
                evidence: Vec::new(),
                provider: Some(ResultProviderId::OPTIMIZATION.stamped()),
            }
        }
        OptimizationResult::InvalidInput { reason } | OptimizationResult::Unevaluated { reason } => {
            unevaluated(reason, ResultProviderId::OPTIMIZATION)
        }
    }
}

fn exact_provider(provider: ResultProviderId) -> DomainMeta {
    DomainMeta {
        status: ComputationStatus::Exact,
        coverage: CoverageStatus::Full,
        symbolic_term: None,
        conditions: Vec::new(),
        diagnostics: Vec::new(),
        evidence: Vec::new(),
        provider: Some(provider.stamped()),
    }
}

fn unevaluated(reason: &Diagnostic, provider: ResultProviderId) -> DomainMeta {
    DomainMeta {
        status: ComputationStatus::Unknown,
        coverage: CoverageStatus::Unsupported,
        symbolic_term: None,
        conditions: Vec::new(),
        diagnostics: vec![reason.clone()],
        evidence: Vec::new(),
        provider: Some(provider.stamped()),
    }
}
