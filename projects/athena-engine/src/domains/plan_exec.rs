//! `DomainPlan` 步骤解释器。
//!
//! Reflector / 领域分派按 [`DomainPlan`] 步骤行走，而非把 `CallDomainProvider`
//! 当作唯一真实动作、其余步骤当作注释。

use athena_types::{Diagnostic, DiagnosticCode};

use crate::{
    domains::{
        calculus::{CalculusResult, CalculusValue},
        dispatch::{DomainRequest, DomainResult},
        plan_normalize::normalize_domain_request,
        plan_select::select_domain_representation,
        planner::{DomainPlan, PlanStep},
        verify_replay::{VerifySnapshot, verify_recompute_domain_result},
        views::SeriesPolynomialView,
    },
    runtime::session::Session,
};

/// 解释一份 [`DomainPlan`] 得到的审计轨迹。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Default)]
pub struct PlanStepReport {
    /// 已成功执行的步骤（按顺序）。
    pub executed: Vec<PlanStep>,
    /// 是否已运行 [`PlanStep::Normalize`]（校验 / 强制转换）。
    pub normalized: bool,
    /// `Normalize` 是否至少改写过一个多项式句柄。
    pub normalize_coerced: bool,
    /// 是否已运行 [`PlanStep::SelectRepresentation`]。
    pub representation_selected: bool,
    /// 所选表示族标签（当已运行 `SelectRepresentation` 时）。
    pub selected_representation: Option<&'static str>,
    /// 是否已运行 [`PlanStep::CallDomainProvider`]。
    pub provider_invoked: bool,
    /// 是否已运行 [`PlanStep::Verify`]。
    pub verified: bool,
    /// 是否已运行 [`PlanStep::MaterializeResult`]。
    pub materialized: bool,
    /// 是否已运行 [`PlanStep::EmitResidual`]。
    pub residual_emitted: bool,
    /// 是否已由 [`PlanStep::CrossDomainView`] 打开视图。
    pub cross_domain_view: bool,
}

impl PlanStepReport {
    /// Owning 复制（[`PlanStep`] 为 `Copy`）。
    pub fn owning_copy(&self) -> Self {
        Self {
            executed: self.executed.clone(),
            normalized: self.normalized,
            normalize_coerced: self.normalize_coerced,
            representation_selected: self.representation_selected,
            selected_representation: self.selected_representation,
            provider_invoked: self.provider_invoked,
            verified: self.verified,
            materialized: self.materialized,
            residual_emitted: self.residual_emitted,
            cross_domain_view: self.cross_domain_view,
        }
    }
}

/// 以单一提供者回调运行 [`DomainPlan`] 步骤（最多调用一次）。
///
/// 引导期语义：
/// - `Normalize` — 校验 `DomainObject` / 项句柄，并将多项式引用强制到规范驻留标识。刷新 `Verify` 快照。
/// - `SelectRepresentation` — 确认请求当前活跃的表示族。
/// - `CallDomainProvider` — 恰好调用一次 `provider`。
/// - `CrossDomainView` — 在提供者输出存在后打开 `TypedView`。
/// - `Verify` — 相对声称结果做独立领域重算。
/// - `MaterializeResult` — 将 `DomainResult` 封存给宿主。
/// - `EmitResidual` — 允许与物化并行或替代物化而完成。
pub fn interpret_domain_plan<F>(
    session: &mut Session,
    plan: &DomainPlan,
    request: DomainRequest,
    provider: F,
) -> Result<(DomainResult, PlanStepReport), Diagnostic>
where
    F: FnOnce(&mut Session, DomainRequest) -> Result<DomainResult, Diagnostic>,
{
    if !plan.steps.iter().any(|s| matches!(s, PlanStep::CallDomainProvider)) {
        return Err(plan_err("plan_missing_CallDomainProvider"));
    }
    if !plan.steps.iter().any(|s| matches!(s, PlanStep::MaterializeResult | PlanStep::EmitResidual)) {
        return Err(plan_err("plan_missing_MaterializeResult_or_EmitResidual"));
    }

    let mut report = PlanStepReport::default();
    let mut result: Option<DomainResult> = None;
    let mut pending_request = Some(request);
    let mut verify_snapshot = VerifySnapshot::from_request(pending_request.as_ref().expect("pending request"));
    let mut provider = Some(provider);

    for step in &plan.steps {
        match *step {
            PlanStep::Normalize => {
                let req = pending_request.take().ok_or_else(|| plan_err("request_already_consumed"))?;
                let outcome = normalize_domain_request(session, req)?;
                verify_snapshot = VerifySnapshot::from_request(&outcome.request);
                report.normalized = true;
                report.normalize_coerced = outcome.coerced;
                pending_request = Some(outcome.request);
                report.executed.push(*step);
            }
            PlanStep::SelectRepresentation => {
                let req = pending_request.as_ref().ok_or_else(|| plan_err("request_already_consumed"))?;
                let selected = select_domain_representation(session, req)?;
                report.representation_selected = true;
                report.selected_representation = Some(selected.family);
                report.executed.push(*step);
            }
            PlanStep::CallDomainProvider => {
                if report.provider_invoked {
                    return Err(plan_err("duplicate_CallDomainProvider"));
                }
                let req = pending_request.take().ok_or_else(|| plan_err("request_already_consumed"))?;
                let call = provider.take().ok_or_else(|| plan_err("provider_callback_missing"))?;
                result = Some(call(session, req)?);
                report.provider_invoked = true;
                report.executed.push(*step);
            }
            PlanStep::CrossDomainView => {
                let current = result.as_ref().ok_or_else(|| plan_err("CrossDomainView_before_provider"))?;
                open_cross_domain_view(session, current)?;
                report.cross_domain_view = true;
                report.executed.push(*step);
            }
            PlanStep::Verify => {
                let current = result.as_ref().ok_or_else(|| plan_err("Verify_before_provider"))?;
                verify_recompute_domain_result(session, &verify_snapshot, current)?;
                report.verified = true;
                report.executed.push(*step);
            }
            PlanStep::MaterializeResult => {
                if result.is_none() {
                    return Err(plan_err("MaterializeResult_without_provider_result"));
                }
                report.materialized = true;
                report.executed.push(*step);
            }
            PlanStep::EmitResidual => {
                report.residual_emitted = true;
                report.executed.push(*step);
            }
        }
    }

    if !report.materialized && !report.residual_emitted {
        return Err(plan_err("plan_did_not_complete_Materialize_or_EmitResidual"));
    }
    let out = result.ok_or_else(|| plan_err("plan_completed_without_DomainResult"))?;
    Ok((out, report))
}

fn plan_err(reason: &'static str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "plan_exec").detail("reason", reason)
}

pub(crate) fn open_cross_domain_view(session: &Session, result: &DomainResult) -> Result<(), Diagnostic> {
    match result {
        DomainResult::Calculus(
            CalculusResult::Exact { value: CalculusValue::Series(series_ref), .. }
            | CalculusResult::Conditional { value: CalculusValue::Series(series_ref), .. }
            | CalculusResult::Unevaluated { expression: CalculusValue::Series(series_ref), .. },
        ) => {
            SeriesPolynomialView::open(&session.series_objects, *series_ref).ok_or_else(|| {
                Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                    .detail("domain", "views")
                    .detail("reason", "missing_series_ref_for_cross_domain_view")
                    .arg("ref", series_ref.0)
            })?;
            Ok(())
        }
        _ => Ok(()),
    }
}
