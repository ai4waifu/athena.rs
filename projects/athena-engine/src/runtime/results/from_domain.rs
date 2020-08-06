//! [`DomainResult`] → [`ComputationResult`] 投影（禁止丢弃领域载荷）。

use athena_types::{ComputationStatus, Condition, Diagnostic, TermId};

use crate::{
    domains::{
        calculus::{CalculusResult, CalculusValue},
        context::DomainExecutionContext,
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
        results::{ComputationResult, CoverageStatus, ResultEvidence, ResultProvenance, ResultProviderId, ResultProviderStamp},
        session::Session,
        values::RuntimeValue,
    },
};

/// 将域结果写入 `ValueStore` / `ComputationResult`（保留完整 `DomainResult`）。
///
/// **禁止**把领域枚举名 `Exact` 自动抬成 [`ComputationStatus::Exact`]。
///
/// - 微积分：仅 `AdmissionGate` journal 已接纳同结果项时抬 `Exact`/`Full`
/// - 图论 / 群 / 域 / 伽罗瓦：摘要或骨架证书 → `Candidate`/`Partial`
/// - 多项式：无结果级 admission 绑定时不得抬升（`Exact` 枚举名 ≠ 已准入）
/// - 数论可信 kernel（如 gcd）可带 provider stamp 保留 `Exact`（非字段伪造路径）
/// - 线性代数：跟 `AlgorithmGuarantee`，禁止机器近似抬 Exact
pub fn computation_from_domain(session: &mut Session, domain: DomainResult) -> ComputationResult {
    let mapped = map_domain_meta(session, &domain);
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

fn map_domain_meta(session: &mut Session, domain: &DomainResult) -> DomainMeta {
    match domain {
        DomainResult::Calculus(r) => map_calculus(session, r),
        DomainResult::NumberTheory(r) => map_number_theory(r),
        DomainResult::Polynomial(r) => map_polynomial(r),
        DomainResult::GroupTheory(r) => map_group(r),
        DomainResult::FieldTheory(r) => map_field(r),
        DomainResult::GaloisTheory(r) => map_galois(r),
        DomainResult::GraphTheory(r) => map_graph(r),
        DomainResult::LinearAlgebra(r) => map_linear_algebra(r),
        DomainResult::Optimization(r) => map_optimization(r),
        DomainResult::Solve(r) => map_solve(r),
    }
}

fn candidate_provider(provider: ResultProviderId) -> DomainMeta {
    DomainMeta {
        status: ComputationStatus::Candidate,
        coverage: CoverageStatus::Partial,
        symbolic_term: None,
        conditions: Vec::new(),
        diagnostics: Vec::new(),
        evidence: Vec::new(),
        provider: Some(provider.stamped()),
    }
}

fn map_calculus(session: &mut Session, result: &CalculusResult<CalculusValue>) -> DomainMeta {
    match result {
        CalculusResult::Exact { value, conditions } => {
            // 领域 Exact ≠ 已准入。仅当 journal 中已有同结果项的 `CalculusRelation` 才抬 Exact/Full。
            let term = calculus_bridge_term(session, value);
            let (status, coverage) = match (term, conditions.is_empty()) {
                (Some(t), true) if calculus_result_admitted(session, t) => (ComputationStatus::Exact, CoverageStatus::Full),
                _ => (ComputationStatus::Candidate, CoverageStatus::Partial),
            };
            DomainMeta {
                status,
                coverage,
                symbolic_term: term,
                conditions: conditions.clone(),
                diagnostics: Vec::new(),
                evidence: Vec::new(),
                provider: Some(ResultProviderId::CALCULUS.stamped()),
            }
        }
        CalculusResult::Conditional { value, conditions } => DomainMeta {
            status: ComputationStatus::Conditional,
            coverage: CoverageStatus::Partial,
            symbolic_term: calculus_bridge_term(session, value),
            conditions: conditions.clone(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::CALCULUS.stamped()),
        },
        CalculusResult::Unevaluated { expression, reason } => DomainMeta {
            status: ComputationStatus::Unknown,
            coverage: CoverageStatus::Unsupported,
            symbolic_term: calculus_bridge_term(session, expression),
            conditions: Vec::new(),
            diagnostics: vec![reason.clone()],
            evidence: Vec::new(),
            provider: Some(ResultProviderId::CALCULUS.stamped()),
        },
    }
}

fn calculus_result_admitted(session: &Session, result_term: TermId) -> bool {
    use crate::reasoning::mgraph::Proposition;
    session.mgraph.semantic.admission_journal().claims().iter().any(|verified| {
        matches!(
            &verified.claim().proposition,
            Proposition::CalculusRelation { result_term: t, .. } if *t == result_term
        )
    })
}

/// Bridge typed calculus payloads to a display/eval `TermId` without dropping the `DomainResult` value.
fn calculus_bridge_term(session: &mut Session, value: &CalculusValue) -> Option<TermId> {
    match value {
        CalculusValue::Expression(term) => Some(*term),
        CalculusValue::Series(r) => {
            let series = session.series_objects.get(*r)?.owning_copy();
            // Empty residual series (unevaluated) must not collapse to `0`.
            if series.terms.is_empty() {
                return None;
            }
            let mut dc = DomainExecutionContext::new(session);
            series.to_term(&mut dc).ok()
        }
        other => {
            let mut dc = DomainExecutionContext::new(session);
            other.materialize_expression(&mut dc).ok()
        }
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
        // 多项式 Exact 枚举名 ≠ 已准入；结果级 admission 绑定前不得抬 Exact/Full。
        PolynomialResult::Exact { .. } => candidate_provider(ResultProviderId::POLYNOMIAL),
        PolynomialResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::POLYNOMIAL),
    }
}

fn map_group(result: &GroupResult) -> DomainMeta {
    match result {
        GroupResult::Exact { .. } => candidate_provider(ResultProviderId::GROUP),
        GroupResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::GROUP),
    }
}

fn map_field(result: &FieldResult) -> DomainMeta {
    match result {
        FieldResult::Exact { .. } => candidate_provider(ResultProviderId::FIELD),
        FieldResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::FIELD),
    }
}

fn map_galois(result: &GaloisResult) -> DomainMeta {
    match result {
        GaloisResult::Exact { .. } => candidate_provider(ResultProviderId::GALOIS),
        GaloisResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::GALOIS),
    }
}

fn map_graph(result: &GraphTheoryResult) -> DomainMeta {
    match result {
        // 图论 L1 多为摘要证书，不得把领域枚举 Exact 抬成 ComputationStatus::Exact/Full。
        GraphTheoryResult::Exact { .. } => candidate_provider(ResultProviderId::GRAPH_THEORY),
        GraphTheoryResult::Unevaluated { reason } => unevaluated(reason, ResultProviderId::GRAPH_THEORY),
    }
}

fn map_linear_algebra(result: &LinearAlgebraResult) -> DomainMeta {
    match result {
        LinearAlgebraResult::Ok { value } => {
            let (status, coverage) = linear_algebra_status_coverage(value);
            DomainMeta {
                status,
                coverage,
                symbolic_term: None,
                conditions: Vec::new(),
                diagnostics: Vec::new(),
                evidence: Vec::new(),
                provider: Some(ResultProviderId::LINEAR_ALGEBRA.stamped()),
            }
        }
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

/// 按值载荷与 [`AlgorithmGuarantee`] 投影顶层状态。禁止把机器近似 `Ok` 抬成 Exact+Full。
fn linear_algebra_status_coverage(value: &crate::domains::linear_algebra::LinearAlgebraValue) -> (ComputationStatus, CoverageStatus) {
    use crate::domains::linear_algebra::{LinearAlgebraValue, SolveDisposition};

    match value {
        LinearAlgebraValue::Matrix(matrix) => {
            if matrix.parent().element.is_machine() {
                // 机器矩阵完整交付：Approximate + Full（非 Partial 截断，非搜索 Candidate）。
                (ComputationStatus::Approximate, CoverageStatus::Full)
            } else {
                (ComputationStatus::Exact, CoverageStatus::Full)
            }
        }
        LinearAlgebraValue::ExactRank(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::MachineRank { guarantee, .. } => algorithm_guarantee_status(*guarantee),
        LinearAlgebraValue::ExactDet(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::ExactTrace(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::ExactRref(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::ExactSolve(r) => {
            let (status, coverage) = algorithm_guarantee_status(r.guarantee);
            match &r.disposition {
                SolveDisposition::Unique | SolveDisposition::Inconsistent => (status, coverage),
                SolveDisposition::Infinite { .. } => (status, CoverageStatus::Partial),
                SolveDisposition::Singular => (ComputationStatus::Partial, CoverageStatus::Partial),
                SolveDisposition::ResourceLimited => (ComputationStatus::ResourceLimited, CoverageStatus::Partial),
            }
        }
        LinearAlgebraValue::MachineSolve(r) => {
            let (status, coverage) = algorithm_guarantee_status(r.guarantee);
            match &r.disposition {
                SolveDisposition::ResourceLimited => (ComputationStatus::ResourceLimited, CoverageStatus::Partial),
                SolveDisposition::Singular => (ComputationStatus::Partial, CoverageStatus::Partial),
                SolveDisposition::Infinite { .. } => (status, CoverageStatus::Partial),
                SolveDisposition::Unique | SolveDisposition::Inconsistent => (status, coverage),
            }
        }
    }
}

fn algorithm_guarantee_status(guarantee: crate::domains::linear_algebra::AlgorithmGuarantee) -> (ComputationStatus, CoverageStatus) {
    use crate::domains::linear_algebra::AlgorithmGuarantee;
    match guarantee {
        AlgorithmGuarantee::Exact => (ComputationStatus::Exact, CoverageStatus::Full),
        AlgorithmGuarantee::Probable => (ComputationStatus::Probable, CoverageStatus::Full),
        // 完整近似 ≠ 搜索候选。状态用 Approximate，覆盖可为 Full。
        AlgorithmGuarantee::Approximate => (ComputationStatus::Approximate, CoverageStatus::Full),
        AlgorithmGuarantee::Partial => (ComputationStatus::Partial, CoverageStatus::Partial),
        AlgorithmGuarantee::Unsupported => (ComputationStatus::Unknown, CoverageStatus::Unsupported),
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
                ComputationStatus::Exact | ComputationStatus::Verified | ComputationStatus::Approximate => CoverageStatus::Full,
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

fn map_solve(result: &crate::domains::solve::SolveResult) -> DomainMeta {
    match result {
        crate::domains::solve::SolveResult::Exact { term } => DomainMeta {
            status: ComputationStatus::Candidate,
            coverage: CoverageStatus::Partial,
            symbolic_term: Some(*term),
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: Some(ResultProviderId::SOLVE.stamped()),
        },
        crate::domains::solve::SolveResult::Unevaluated { expression, reason } => DomainMeta {
            status: ComputationStatus::Unknown,
            coverage: CoverageStatus::Unsupported,
            symbolic_term: Some(*expression),
            conditions: Vec::new(),
            diagnostics: vec![reason.clone()],
            evidence: Vec::new(),
            provider: Some(ResultProviderId::SOLVE.stamped()),
        },
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
