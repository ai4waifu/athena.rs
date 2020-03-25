//! 排队的 `DomainPlan` 执行（· `pending_plans` bootstrap）。
//!
//! 计划入队时不伪造事实。执行仍经域 provider 与 [`AdmissionGate`] 接纳辅助 — 禁止直接写 ExactUF。
//!
//! [`PlanBinding`] 把排队计划绑到可核验的请求指纹，避免调用方提供的 [`DomainRequest`] 与义务静默错配。

use athena_ir::fnv1a64;
use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::CalculusRequest,
        dispatch::{DomainRequest, DomainResult, call_domain_provider},
        plan_exec::interpret_domain_plan,
        planner::DomainPlan,
        polynomial::{cache_key_for_request, execute_polynomial_mgraph},
    },
    reasoning::mgraph::{
        core::refs::{PredicateId, TheoryContextId, predicates},
        obligation::ProofObligation,
        semantic_entry::try_admit_calculus_exact,
    },
    runtime::session::Session,
};

/// 排队计划与可执行 [`DomainRequest`] 之间的可核验链接。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlanBinding {
    /// 已由唤醒调度、尚无请求（调用方须谨慎绑定）。
    #[default]
    Unbound,
    /// 稳定指纹门：仅当提供的请求匹配时才执行。
    Fingerprint {
        /// 请求期望的理论上下文。
        theory: TheoryContextId,
        /// 义务 / 关系族上期望的谓词。
        predicate: PredicateId,
        /// 请求身份指纹（由域定义）。
        request_fingerprint: u64,
    },
}

/// Reflector 选出、等待绑定 [`DomainRequest`] 的计划。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub struct QueuedPlan {
    /// `DomainPlan` 步骤（要能跑必须含 `CallDomainProvider`）。
    pub plan: DomainPlan,
    /// 产生 `NeedComputation` 的义务。
    pub obligation: ProofObligation,
    /// 请求 / 目标绑定（指纹门或未绑定）。
    pub binding: PlanBinding,
}

impl QueuedPlan {
    /// Owning 复制。
    pub fn owning_copy(&self) -> Self {
        Self { plan: self.plan.owning_copy(), obligation: self.obligation.owning_copy(), binding: self.binding }
    }

    /// 无请求指纹地入队（唤醒路径）。
    pub fn unbound(plan: DomainPlan, obligation: ProofObligation) -> Self {
        Self { plan, obligation, binding: PlanBinding::Unbound }
    }

    /// 用由 `request` 导出的指纹入队。
    pub fn bound(session: &Session, plan: DomainPlan, obligation: ProofObligation, request: &DomainRequest) -> Self {
        let binding = plan_binding_for_request(session, request, &obligation).unwrap_or(PlanBinding::Unbound);
        Self { plan, obligation, binding }
    }
}

/// 为微积分 / 多项式请求导出 [`PlanBinding`] 指纹。
pub fn plan_binding_for_request(session: &Session, request: &DomainRequest, obligation: &ProofObligation) -> Option<PlanBinding> {
    match request {
        DomainRequest::Polynomial(req) => {
            let key = cache_key_for_request(req, &session.rings, &session.polynomial_objects).ok()?;
            Some(PlanBinding::Fingerprint {
                theory: TheoryContextId::POLYNOMIAL,
                predicate: predicates::POLYNOMIAL_RESULT,
                request_fingerprint: key.fingerprint(),
            })
        }
        DomainRequest::Calculus(calc) => {
            let (predicate, fingerprint) = calculus_binding(calc)?;
            Some(PlanBinding::Fingerprint { theory: TheoryContextId::CALCULUS, predicate, request_fingerprint: fingerprint })
        }
        _ => {
            // 回退：混入义务身份，使尚未单独建模的域在义务有已知对象时仍有门控。
            if obligation.known_objects.is_empty() {
                return None;
            }
            let mut state = fnv1a64(b"athena.plan-binding.obligation");
            mix_u64(&mut state, u64::from(obligation.predicate.0));
            mix_u64(&mut state, u64::from(obligation.scope.0));
            for obj in &obligation.known_objects {
                mix_u64(&mut state, u64::from(obj.theory.0));
                mix_u64(&mut state, obj.fingerprint);
            }
            Some(PlanBinding::Fingerprint {
                theory: obligation.known_objects.first().map(|o| o.theory).unwrap_or(TheoryContextId::DEFAULT),
                predicate: obligation.predicate,
                request_fingerprint: state,
            })
        }
    }
}

fn calculus_binding(request: &CalculusRequest) -> Option<(PredicateId, u64)> {
    let mut state = fnv1a64(b"athena.plan-binding.calculus");
    match request {
        CalculusRequest::Derivative { expression, variable, order, .. } => {
            mix_u64(&mut state, u64::from(expression.0));
            mix_u64(&mut state, u64::from(variable.0));
            mix_u64(&mut state, derivative_order_tag(*order));
            Some((predicates::DERIVATIVE_OF, state))
        }
        CalculusRequest::Integral { expression, variable, .. } | CalculusRequest::DefiniteIntegral { expression, variable, .. } => {
            mix_u64(&mut state, u64::from(expression.0));
            mix_u64(&mut state, u64::from(variable.0));
            Some((predicates::INTEGRAL_OF, state))
        }
        CalculusRequest::Series { expression, variable, .. }
        | CalculusRequest::Laurent { expression, variable, .. }
        | CalculusRequest::Asymptotic { expression, variable, .. } => {
            mix_u64(&mut state, u64::from(expression.0));
            mix_u64(&mut state, u64::from(variable.0));
            Some((predicates::SERIES_EXPANSION, state))
        }
        _ => None,
    }
}

fn derivative_order_tag(order: crate::domains::calculus::DerivativeOrder) -> u64 {
    match order {
        crate::domains::calculus::DerivativeOrder::First => 1,
        crate::domains::calculus::DerivativeOrder::Repeated(n) => 2 + u64::from(n),
    }
}

fn mix_u64(state: &mut u64, v: u64) {
    *state ^= v;
    *state = state.wrapping_mul(0x0000_0100_0000_01b3);
}

/// 拒绝与指纹门排队计划不匹配的请求。
pub fn verify_plan_binding(
    session: &Session,
    binding: &PlanBinding,
    obligation: &ProofObligation,
    request: &DomainRequest,
) -> Result<(), Diagnostic> {
    let PlanBinding::Fingerprint { theory, predicate, request_fingerprint } = binding
    else {
        return Ok(());
    };
    if obligation.predicate != *predicate {
        return Err(binding_mismatch("obligation_predicate"));
    }
    let Some(PlanBinding::Fingerprint { theory: got_theory, predicate: got_predicate, request_fingerprint: got_fp }) =
        plan_binding_for_request(session, request, obligation)
    else {
        return Err(binding_mismatch("request_unfingerprintable"));
    };
    if got_theory != *theory || got_predicate != *predicate || got_fp != *request_fingerprint {
        return Err(binding_mismatch("request_fingerprint"));
    }
    Ok(())
}

fn binding_mismatch(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("domain", "pending_plans")
        .detail("reason", "plan_binding_mismatch")
        .detail("detail", reason)
}

/// 用调用方绑定的请求执行一条排队计划（精确结果走 `AdmissionGate`）。
///
/// 经 [`interpret_domain_plan`] 走 [`DomainPlan`]。多项式 provider 用
/// `execute_polynomial_mgraph`；微积分精确结果在 materialize 后接纳。
pub fn execute_queued_plan(session: &mut Session, queued: &QueuedPlan, request: DomainRequest) -> Result<DomainResult, Diagnostic> {
    verify_plan_binding(session, &queued.binding, &queued.obligation, &request)?;
    let obligation = queued.obligation.owning_copy();
    let (result, _report) = interpret_domain_plan(session, &queued.plan, request, |session, req| match req {
        DomainRequest::Polynomial(poly_req) => {
            let poly = execute_polynomial_mgraph(poly_req, &session.rings, &session.polynomial_objects, &mut session.mgraph);
            Ok(DomainResult::Polynomial(poly))
        }
        other => call_domain_provider(session, other),
    })?;
    try_admit_calculus_exact(session, &obligation, &result);
    Ok(result)
}

/// 弹出并执行队首排队计划（带绑定请求）。
///
/// 队列空时返回 `Ok(None)`。provider / 接纳 / 绑定失败时计划留在队首
/// （结构畸形、缺 `CallDomainProvider` 的计划除外，会被丢弃）。
pub fn run_next_queued_plan(session: &mut Session, request: DomainRequest) -> Result<Option<DomainResult>, Diagnostic> {
    let Some(queued) = session.mgraph.operational.pending_plans.first().map(QueuedPlan::owning_copy)
    else {
        return Ok(None);
    };
    match execute_queued_plan(session, &queued, request) {
        Ok(result) => {
            let _ = session.mgraph.operational.pending_plans.remove(0);
            Ok(Some(result))
        }
        Err(err) => {
            // 丢弃解释器因结构拒绝的畸形计划。
            let reason = err.details.get("reason").map(|v| v.to_string());
            if matches!(reason.as_deref(), Some("plan_missing_CallDomainProvider") | Some("plan_missing_MaterializeResult_or_EmitResidual")) {
                let _ = session.mgraph.operational.pending_plans.remove(0);
            }
            Err(err)
        }
    }
}

/// 批量执行排队计划，每条配下一个绑定请求。
///
/// 遇到第一个 provider / 绑定错误即停（该计划留在队首）。超出队列长度的多余请求忽略。
/// 请求用尽后，剩余计划继续排队。
pub fn run_queued_plans(session: &mut Session, requests: impl IntoIterator<Item = DomainRequest>) -> Result<QueuedPlanBatchReport, Diagnostic> {
    let mut report = QueuedPlanBatchReport::default();
    for request in requests {
        if session.mgraph.operational.pending_plans.is_empty() {
            break;
        }
        match run_next_queued_plan(session, request)? {
            Some(result) => {
                report.executed = report.executed.saturating_add(1);
                report.results.push(result);
            }
            None => break,
        }
    }
    report.remaining = session.mgraph.operational.pending_plans.len() as u32;
    Ok(report)
}

/// 批量执行排队计划的报告。
#[derive(Debug, PartialEq, Default)]
pub struct QueuedPlanBatchReport {
    /// 本次成功执行的计划数。
    pub executed: u32,
    /// 仍在队列中等待的计划数。
    pub remaining: u32,
    /// 按执行顺序的域结果。
    pub results: Vec<DomainResult>,
}
