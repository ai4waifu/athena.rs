//! Living `04` 可观测编译阶段投影（迁移切片）。
//!
//! 观测视图由 [`super::stages`] 具名程序投影而来。
//! Request / Plan 在 lowering 前真实产出；Semantic / CFG SSA 仍过渡物化自 module。
//! 禁止把 dump 当成第二套执行路径。

use std::fmt::Write as _;

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    api::request::AthenaRequest,
    execution::ir::{ExecutionModule, verify_module},
};

use super::stages::{
    self, CfgSsaProgram, CompileStageKind, PlanIntent, PlanProgram, RequestProgram, SemanticProgram, StageFingerprint, canonicalize_request,
    materialize_cfg_ssa, materialize_semantic, plan_from_request, render_cfg_text,
};

/// 兼容旧名：Request 阶段视图即 [`RequestProgram`]。
pub type RequestStageView = RequestProgram;
/// 兼容旧名：Plan 阶段视图即 [`PlanProgram`]。
pub type PlanStageView = PlanProgram;
/// 兼容旧名：Semantic 阶段视图即 [`SemanticProgram`]。
pub type SemanticStageView = SemanticProgram;
/// 兼容旧名：CFG SSA 阶段视图即 [`CfgSsaProgram`]。
pub type CfgSsaStageView = CfgSsaProgram;

/// 一次编译的四阶段观测（由具名程序投影）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileObservation {
    /// Request 程序。
    pub request: RequestProgram,
    /// Plan 程序。
    pub plan: PlanProgram,
    /// Semantic 程序。
    pub semantic: SemanticProgram,
    /// CFG SSA 程序。
    pub cfg_ssa: CfgSsaProgram,
}

impl CompileObservation {
    /// 由具名阶段程序构造观测。
    pub fn from_programs(request: RequestProgram, plan: PlanProgram, semantic: SemanticProgram, cfg_ssa: CfgSsaProgram) -> Self {
        Self { request, plan, semantic, cfg_ssa }
    }

    /// 渲染四阶段稳定观测文本。
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ =
            writeln!(out, "stage request kind={} term={:?} fp={:#x}", self.request.kind, self.request.term_index, self.request.fingerprint.0);
        let _ = writeln!(
            out,
            "stage plan intent={:?} provider_required={} fp={:#x}",
            self.plan.intent, self.plan.provider_required, self.plan.fingerprint.0
        );
        let _ = writeln!(
            out,
            "stage semantic ops={} effects={} providers={} fp={:#x}",
            self.semantic.operations.len(),
            self.semantic.effect_edge_count,
            self.semantic.provider_call_count,
            self.semantic.fingerprint.0
        );
        for (idx, op) in self.semantic.operations.iter().enumerate() {
            let _ =
                writeln!(out, "  sem[{}] {} : {} effect_in={:?} effect_out={:?}", idx, op.kind, op.result_type, op.effect_in, op.effect_out);
        }
        let _ = writeln!(
            out,
            "stage cfg_ssa regions={} blocks={} entry={} module_fp={:#x} fp={:#x}",
            self.cfg_ssa.region_count,
            self.cfg_ssa.block_count,
            self.cfg_ssa.entry_block,
            self.cfg_ssa.module_fingerprint.0,
            self.cfg_ssa.fingerprint.0
        );
        out.push_str(&self.cfg_ssa.text);
        out
    }
}

/// 由请求与已校验 module 构造四阶段观测（Request/Plan 先于 module 决策）。
pub fn observe_compile(request: &AthenaRequest, module: &ExecutionModule) -> Result<CompileObservation> {
    let request_prog = canonicalize_request(request);
    let plan_prog = plan_from_request(&request_prog);
    let semantic = materialize_semantic(module);
    let cfg_ssa = materialize_cfg_ssa(module);
    let observation = CompileObservation::from_programs(request_prog, plan_prog, semantic, cfg_ssa);
    verify_observation(&observation, module)?;
    Ok(observation)
}

/// Request 阶段（委托 canonicalize）。
pub fn dump_request(request: &AthenaRequest) -> RequestStageView {
    canonicalize_request(request)
}

/// Plan 阶段（先 canonicalize，再 plan）。
pub fn dump_plan(request: &AthenaRequest) -> PlanStageView {
    plan_from_request(&canonicalize_request(request))
}

/// Semantic 阶段（委托物化）。
pub fn dump_semantic(module: &ExecutionModule) -> SemanticStageView {
    materialize_semantic(module)
}

/// CFG SSA 阶段（委托物化）。
pub fn dump_cfg_ssa(module: &ExecutionModule) -> CfgSsaStageView {
    materialize_cfg_ssa(module)
}

/// 校验四阶段观测与 module 一致，并复跑结构 verifier。
pub fn verify_observation(observation: &CompileObservation, module: &ExecutionModule) -> Result<()> {
    verify_module(module)?;

    let expected_plan = dump_plan_intent_for_kind(observation.request.kind)?;
    if observation.plan.intent != expected_plan {
        return Err(stage_diag(CompileStageKind::Plan, "plan_intent_mismatch"));
    }
    if observation.plan.provider_required != matches!(observation.plan.intent, PlanIntent::DomainProvider) {
        return Err(stage_diag(CompileStageKind::Plan, "provider_flag_mismatch"));
    }
    if observation.plan.request_fingerprint != observation.request.fingerprint {
        return Err(stage_diag(CompileStageKind::Plan, "request_plan_chain_mismatch"));
    }

    if observation.cfg_ssa.module_fingerprint != module.fingerprint {
        return Err(stage_diag(CompileStageKind::CfgSsa, "module_fingerprint_mismatch"));
    }
    if observation.cfg_ssa.region_count != module.regions.len() {
        return Err(stage_diag(CompileStageKind::CfgSsa, "region_count_mismatch"));
    }
    let block_count: usize = module.regions.iter().map(|r| r.blocks.len()).sum();
    if observation.cfg_ssa.block_count != block_count {
        return Err(stage_diag(CompileStageKind::CfgSsa, "block_count_mismatch"));
    }
    if observation.cfg_ssa.text != render_cfg_text(module) {
        return Err(stage_diag(CompileStageKind::CfgSsa, "cfg_text_drift"));
    }

    let recomputed_semantic = materialize_semantic(module);
    if observation.semantic.fingerprint != recomputed_semantic.fingerprint {
        return Err(stage_diag(CompileStageKind::Semantic, "semantic_fingerprint_drift"));
    }
    let recomputed_request = canonicalize_request_fingerprint(observation.request.kind, observation.request.term_index);
    if observation.request.fingerprint != recomputed_request {
        return Err(stage_diag(CompileStageKind::Request, "request_fingerprint_drift"));
    }
    Ok(())
}

fn dump_plan_intent_for_kind(kind: &str) -> Result<PlanIntent> {
    match kind {
        "Term" => Ok(PlanIntent::EvaluateTerm),
        "Control" => Ok(PlanIntent::RunControl),
        "Command" => Ok(PlanIntent::SessionCommand),
        "Goal" => Ok(PlanIntent::DomainProvider),
        _ => Err(stage_diag(CompileStageKind::Request, "unknown_request_kind")),
    }
}

fn canonicalize_request_fingerprint(kind: &str, term_index: Option<u32>) -> StageFingerprint {
    stages::stage_fingerprint(CompileStageKind::Request, |h| {
        use std::hash::Hash;
        kind.hash(h);
        term_index.hash(h);
    })
}

fn stage_diag(stage: CompileStageKind, reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("component", "compiler_dump")
        .detail("stage", format!("{stage:?}"))
        .detail("reason", reason)
}
