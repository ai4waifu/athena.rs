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
/// - 微积分：仅 `AdmissionGate` journal 已接纳同结果项时抬 `Exact`/`Full`，并附 `AdmittedRelation`
/// - 图论 / 群 / 域 / 伽罗瓦：摘要或骨架证书 → `Candidate`/`Partial`
/// - 多项式：仅 journal 已接纳且 operational verified 缓存与值对齐时抬 `Exact`/`Full`（附 `AdmittedRelation`）
/// - 数论可信 kernel（如 gcd）可带 provider stamp 保留 `Exact`（非字段伪造路径）
/// - 线性代数：跟 `AlgorithmGuarantee`，禁止机器近似抬 Exact
pub fn computation_from_domain(session: &mut Session, mut domain: DomainResult) -> ComputationResult {
    // Living 16: Matrix/Dot envelopes published into the session must carry store identity.
    attach_linear_algebra_matrix_refs(session, &mut domain);
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

/// Intern owned `Matrix` / `Dot` envelopes so published `MatrixResult` carries `matrix_ref` + revision.
fn attach_linear_algebra_matrix_refs(session: &mut Session, domain: &mut DomainResult) {
    let DomainResult::LinearAlgebra(LinearAlgebraResult::Ok { value }) = domain else {
        return;
    };
    let envelope = match value {
        crate::domains::linear_algebra::LinearAlgebraValue::Matrix(envelope)
        | crate::domains::linear_algebra::LinearAlgebraValue::Dot(envelope) => envelope,
        _ => return,
    };
    if envelope.matrix_ref.is_some() {
        return;
    }
    let matrix_ref = session.matrix_objects.intern(envelope.value.owning_copy());
    let revision = session.matrix_objects.revision(matrix_ref).unwrap_or(0);
    envelope.matrix_ref = Some(matrix_ref);
    envelope.revision = Some(revision);
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
        DomainResult::Polynomial(r) => map_polynomial(session, r),
        DomainResult::GroupTheory(r) => map_group(r),
        DomainResult::FieldTheory(r) => map_field(r),
        DomainResult::GaloisTheory(r) => map_galois(r),
        DomainResult::GraphTheory(r) => map_graph(r),
        DomainResult::LinearAlgebra(r) => map_linear_algebra(session, r),
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
            let admitted = term.filter(|_| conditions.is_empty()).and_then(|t| calculus_admitted_fact(session, t));
            let (status, coverage, evidence) = match admitted {
                Some(fact) => (
                    ComputationStatus::Exact,
                    CoverageStatus::Full,
                    vec![ResultEvidence::AdmittedRelation { fact }],
                ),
                None => (ComputationStatus::Candidate, CoverageStatus::Partial, Vec::new()),
            };
            DomainMeta {
                status,
                coverage,
                symbolic_term: term,
                conditions: conditions.clone(),
                diagnostics: Vec::new(),
                evidence,
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

fn calculus_admitted_fact(session: &Session, result_term: TermId) -> Option<crate::reasoning::mgraph::FactId> {
    use crate::reasoning::mgraph::{FactId, Proposition};
    session.mgraph.semantic.admission_journal().claims().iter().enumerate().find_map(|(i, verified)| {
        match &verified.claim().proposition {
            Proposition::CalculusRelation { result_term: t, .. } if *t == result_term => Some(FactId(i as u64)),
            _ => None,
        }
    })
}

/// journal 中的 `PolynomialResult` 命题须与 operational verified 缓存中的同指纹 Exact 值对齐。
fn polynomial_admitted_fact(
    session: &Session,
    value: &crate::domains::polynomial::PolynomialDomainValue,
) -> Option<crate::reasoning::mgraph::FactId> {
    use crate::reasoning::mgraph::{FactId, Proposition, PolynomialCacheTier};
    use crate::domains::polynomial::PolynomialResult;

    for (i, verified) in session.mgraph.semantic.admission_journal().claims().iter().enumerate() {
        let Proposition::PolynomialResult { request_fingerprint, .. } = verified.claim().proposition
        else {
            continue;
        };
        let Some(entry) = session
            .mgraph
            .operational
            .result_cache
            .polynomial
            .get_by_request_fingerprint(request_fingerprint)
        else {
            continue;
        };
        if entry.tier != PolynomialCacheTier::Verified {
            continue;
        }
        if let PolynomialResult::Exact { value: cached } = &entry.result {
            if cached == value {
                return Some(FactId(i as u64));
            }
        }
    }
    None
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

fn map_polynomial(session: &Session, result: &PolynomialResult) -> DomainMeta {
    match result {
        // 多项式 Exact 枚举名 ≠ 已准入；仅 journal + verified 缓存值对齐后抬 Exact/Full。
        PolynomialResult::Exact { value } => match polynomial_admitted_fact(session, value) {
            Some(fact) => DomainMeta {
                status: ComputationStatus::Exact,
                coverage: CoverageStatus::Full,
                symbolic_term: None,
                conditions: Vec::new(),
                diagnostics: Vec::new(),
                evidence: vec![ResultEvidence::AdmittedRelation { fact }],
                provider: Some(ResultProviderId::POLYNOMIAL.stamped()),
            },
            None => candidate_provider(ResultProviderId::POLYNOMIAL),
        },
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

fn map_linear_algebra(session: &mut Session, result: &LinearAlgebraResult) -> DomainMeta {
    match result {
        LinearAlgebraResult::Ok { value } => {
            let (status, coverage) = linear_algebra_status_coverage(value);
            // 发布时即写入可渲染项，避免宿主 evaluate 成功但 toString 因缺 symbolic_term 硬失败。
            let symbolic_term = crate::execution::reference::linear_algebra_value_symbolic_term(session, value);
            let (diagnostics, evidence) = linear_algebra_envelope_meta(value);
            DomainMeta {
                status,
                coverage,
                symbolic_term,
                conditions: Vec::new(),
                diagnostics,
                evidence,
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

/// Pull Living 16 `MatrixResult` diagnostics / residual / conditioning onto the session result.
fn linear_algebra_envelope_meta(
    value: &crate::domains::linear_algebra::LinearAlgebraValue,
) -> (Vec<Diagnostic>, Vec<ResultEvidence>) {
    use crate::domains::linear_algebra::LinearAlgebraValue;

    let envelope = match value {
        LinearAlgebraValue::Matrix(envelope) | LinearAlgebraValue::Dot(envelope) => envelope,
        _ => return (Vec::new(), Vec::new()),
    };
    let diagnostics = envelope.diagnostics.clone();
    let mut evidence = Vec::new();
    if let Some(residual_inf) = envelope.residual_inf {
        let mut summary = format!("residual_inf={residual_inf}");
        if let Some(conditioning) = envelope.conditioning {
            summary.push_str(&format!(" conditioning={conditioning}"));
        }
        evidence.push(ResultEvidence::TrustedKernelSummary {
            provider: ResultProviderId::LINEAR_ALGEBRA,
            summary,
        });
    } else if let Some(conditioning) = envelope.conditioning {
        evidence.push(ResultEvidence::TrustedKernelSummary {
            provider: ResultProviderId::LINEAR_ALGEBRA,
            summary: format!("conditioning={conditioning}"),
        });
    }
    if let (Some(matrix_ref), Some(revision)) = (envelope.matrix_ref, envelope.revision) {
        evidence.push(ResultEvidence::TrustedKernelSummary {
            provider: ResultProviderId::LINEAR_ALGEBRA,
            summary: format!("matrix_ref={} revision={revision}", matrix_ref.0),
        });
    }
    evidence.push(ResultEvidence::TrustedKernelSummary {
        provider: ResultProviderId::LINEAR_ALGEBRA,
        summary: format!(
            "shape={}x{} element_domain={:?} guarantee={:?}",
            envelope.shape.rows, envelope.shape.cols, envelope.element_domain, envelope.guarantee
        ),
    });
    (diagnostics, evidence)
}

/// 按值载荷与 [`AlgorithmGuarantee`] 投影顶层状态。禁止把机器近似 `Ok` 抬成 Exact+Full。
fn linear_algebra_status_coverage(value: &crate::domains::linear_algebra::LinearAlgebraValue) -> (ComputationStatus, CoverageStatus) {
    use crate::domains::linear_algebra::{LinearAlgebraValue, SolveDisposition};

    match value {
        LinearAlgebraValue::Matrix(envelope) => algorithm_guarantee_status(envelope.guarantee),
        LinearAlgebraValue::ExactRank(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::MachineRank { guarantee, .. } => algorithm_guarantee_status(*guarantee),
        LinearAlgebraValue::ExactDet(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::ExactTrace(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::ExactNorm(r) => algorithm_guarantee_status(r.guarantee),
        LinearAlgebraValue::Dot(envelope) => algorithm_guarantee_status(envelope.guarantee),
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
        crate::domains::solve::SolveResult::Exact { term, coverage } => {
            let (status, result_coverage) = solve_coverage_to_result(coverage);
            DomainMeta {
                status,
                coverage: result_coverage,
                symbolic_term: Some(*term),
                conditions: Vec::new(),
                diagnostics: Vec::new(),
                evidence: vec![ResultEvidence::TrustedKernelSummary {
                    provider: ResultProviderId::SOLVE,
                    summary: format!("solution_rules coverage={}", solve_coverage_name(coverage)),
                }],
                provider: Some(ResultProviderId::SOLVE.stamped()),
            }
        }
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

/// 解域覆盖 → 结果层状态 / 覆盖。禁止把子集根集抬成 Full。
fn solve_coverage_to_result(coverage: &crate::domains::solve::CoverageStatus) -> (ComputationStatus, CoverageStatus) {
    use crate::domains::solve::CoverageStatus as SolveCoverage;
    match coverage {
        SolveCoverage::Complete | SolveCoverage::CompleteUnderAssumptions => (ComputationStatus::Candidate, CoverageStatus::Full),
        SolveCoverage::CertifiedSubset
        | SolveCoverage::CertifiedSuperset
        | SolveCoverage::LocalOnly
        | SolveCoverage::Probable => (ComputationStatus::Candidate, CoverageStatus::Partial),
        SolveCoverage::ResourceLimited { .. } => (ComputationStatus::ResourceLimited, CoverageStatus::Partial),
        SolveCoverage::Unsupported => (ComputationStatus::Unknown, CoverageStatus::Unsupported),
        SolveCoverage::Invalid => (ComputationStatus::Invalid, CoverageStatus::Unsupported),
    }
}

fn solve_coverage_name(coverage: &crate::domains::solve::CoverageStatus) -> &'static str {
    use crate::domains::solve::CoverageStatus as SolveCoverage;
    match coverage {
        SolveCoverage::Complete => "Complete",
        SolveCoverage::CompleteUnderAssumptions => "CompleteUnderAssumptions",
        SolveCoverage::CertifiedSubset => "CertifiedSubset",
        SolveCoverage::CertifiedSuperset => "CertifiedSuperset",
        SolveCoverage::LocalOnly => "LocalOnly",
        SolveCoverage::Probable => "Probable",
        SolveCoverage::ResourceLimited { .. } => "ResourceLimited",
        SolveCoverage::Unsupported => "Unsupported",
        SolveCoverage::Invalid => "Invalid",
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
