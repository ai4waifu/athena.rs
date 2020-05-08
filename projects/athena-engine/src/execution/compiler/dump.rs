//! Living `04` 可观测编译阶段 dump（迁移切片 1）。
//!
//! 在现有 fused `ExecutionCompiler` 上投影 Request / Plan / Semantic / CFG SSA
//! 视图。它们尚非独立 IR 类型，但必须具备稳定文本、fingerprint 与轻量 verifier。
//! 禁止把 dump 当成第二套执行路径。

use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write as _,
    hash::{Hash, Hasher},
};

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{
    api::request::AthenaRequest,
    execution::ir::{
        ExecutionModule, ExecutionValueType, ModuleFingerprint, OperationKind, Terminator, verify_module,
    },
};

/// 编译管线阶段种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompileStageKind {
    /// Request IR 视图。
    Request,
    /// Plan IR 视图。
    Plan,
    /// Semantic IR 视图。
    Semantic,
    /// CFG SSA IR 视图。
    CfgSsa,
}

/// 阶段级结构指纹（不是 `TermId` 下标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StageFingerprint(pub u64);

/// Request 阶段可观测视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestStageView {
    /// `AthenaRequest::kind_name`。
    pub kind: &'static str,
    /// `Term` 请求时的项下标（仅观测，不作跨 session 身份）。
    pub term_index: Option<u32>,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
}

/// Plan 意图（对应 Living `04` Plan IR 粗粒度决策）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanIntent {
    /// 符号项求值。
    EvaluateTerm,
    /// 控制流计划。
    RunControl,
    /// 会话命令。
    SessionCommand,
    /// 领域 provider 调度。
    DomainProvider,
}

/// Plan 阶段可观测视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanStageView {
    /// 计划意图。
    pub intent: PlanIntent,
    /// 是否需要 provider / domain 载荷。
    pub provider_required: bool,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
}

/// Semantic 操作摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticOpSummary {
    /// 封闭操作码名。
    pub kind: &'static str,
    /// 静态结果类型名。
    pub result_type: &'static str,
    /// 入边 effect token（若有）。
    pub effect_in: Option<u32>,
    /// 出边 effect token（若有）。
    pub effect_out: Option<u32>,
}

/// Semantic 阶段可观测视图（由已形成的 module 投影）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticStageView {
    /// 按块遍历顺序的操作摘要。
    pub operations: Vec<SemanticOpSummary>,
    /// effect 边数量。
    pub effect_edge_count: usize,
    /// provider call 描述符数量。
    pub provider_call_count: usize,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
}

/// CFG SSA 阶段可观测视图。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgSsaStageView {
    /// region 数。
    pub region_count: usize,
    /// 全部 block 数。
    pub block_count: usize,
    /// 入口 block 下标（首 region）。
    pub entry_block: u32,
    /// 稳定文本 dump。
    pub text: String,
    /// module 结构指纹。
    pub module_fingerprint: ModuleFingerprint,
    /// 本阶段指纹（绑定文本与 module 指纹）。
    pub fingerprint: StageFingerprint,
}

/// 一次编译的四阶段观测。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileObservation {
    /// Request 视图。
    pub request: RequestStageView,
    /// Plan 视图。
    pub plan: PlanStageView,
    /// Semantic 视图。
    pub semantic: SemanticStageView,
    /// CFG SSA 视图。
    pub cfg_ssa: CfgSsaStageView,
}

impl CompileObservation {
    /// 渲染四阶段稳定观测文本。
    pub fn render(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "stage request kind={} term={:?} fp={:#x}",
            self.request.kind, self.request.term_index, self.request.fingerprint.0
        );
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
            let _ = writeln!(
                out,
                "  sem[{}] {} : {} effect_in={:?} effect_out={:?}",
                idx, op.kind, op.result_type, op.effect_in, op.effect_out
            );
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

/// 由请求与已校验 module 构造四阶段观测。
pub fn observe_compile(request: &AthenaRequest, module: &ExecutionModule) -> Result<CompileObservation> {
    let observation = CompileObservation {
        request: dump_request(request),
        plan: dump_plan(request),
        semantic: dump_semantic(module),
        cfg_ssa: dump_cfg_ssa(module),
    };
    verify_observation(&observation, module)?;
    Ok(observation)
}

/// Request 阶段 dump。
pub fn dump_request(request: &AthenaRequest) -> RequestStageView {
    let term_index = match request {
        AthenaRequest::Term(term) => Some(term.0),
        _ => None,
    };
    let kind = request.kind_name();
    let fingerprint = stage_fingerprint(CompileStageKind::Request, |h| {
        kind.hash(h);
        term_index.hash(h);
    });
    RequestStageView { kind, term_index, fingerprint }
}

/// Plan 阶段 dump。
pub fn dump_plan(request: &AthenaRequest) -> PlanStageView {
    let (intent, provider_required) = match request {
        AthenaRequest::Term(_) => (PlanIntent::EvaluateTerm, false),
        AthenaRequest::Control(_) => (PlanIntent::RunControl, false),
        AthenaRequest::Command(_) => (PlanIntent::SessionCommand, false),
        AthenaRequest::Goal(_) => (PlanIntent::DomainProvider, true),
    };
    let fingerprint = stage_fingerprint(CompileStageKind::Plan, |h| {
        core::mem::discriminant(&intent).hash(h);
        provider_required.hash(h);
    });
    PlanStageView { intent, provider_required, fingerprint }
}

/// Semantic 阶段 dump（从 module 操作投影）。
pub fn dump_semantic(module: &ExecutionModule) -> SemanticStageView {
    let mut operations = Vec::new();
    for region in &module.regions {
        for block in &region.blocks {
            for op in &block.operations {
                operations.push(SemanticOpSummary {
                    kind: operation_kind_name(&op.kind),
                    result_type: value_type_name(&op.result_type),
                    effect_in: op.effect_in.map(|t| t.0),
                    effect_out: op.effect_out.map(|t| t.0),
                });
            }
        }
    }
    let effect_edge_count = module.effect_edges.len();
    let provider_call_count = module.provider_calls.len();
    let fingerprint = stage_fingerprint(CompileStageKind::Semantic, |h| {
        operations.len().hash(h);
        for op in &operations {
            op.kind.hash(h);
            op.result_type.hash(h);
            op.effect_in.hash(h);
            op.effect_out.hash(h);
        }
        effect_edge_count.hash(h);
        provider_call_count.hash(h);
    });
    SemanticStageView {
        operations,
        effect_edge_count,
        provider_call_count,
        fingerprint,
    }
}

/// CFG SSA 阶段 dump。
pub fn dump_cfg_ssa(module: &ExecutionModule) -> CfgSsaStageView {
    let text = render_cfg_text(module);
    let region_count = module.regions.len();
    let block_count: usize = module.regions.iter().map(|r| r.blocks.len()).sum();
    let entry_block = module.regions.first().map(|r| r.entry.0).unwrap_or(0);
    let module_fingerprint = module.fingerprint;
    let fingerprint = stage_fingerprint(CompileStageKind::CfgSsa, |h| {
        text.hash(h);
        module_fingerprint.0.hash(h);
        region_count.hash(h);
        block_count.hash(h);
        entry_block.hash(h);
    });
    CfgSsaStageView {
        region_count,
        block_count,
        entry_block,
        text,
        module_fingerprint,
        fingerprint,
    }
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

    let recomputed_semantic = dump_semantic(module);
    if observation.semantic.fingerprint != recomputed_semantic.fingerprint {
        return Err(stage_diag(CompileStageKind::Semantic, "semantic_fingerprint_drift"));
    }
    if observation.request.fingerprint != dump_request_fingerprint(observation.request.kind, observation.request.term_index) {
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

fn dump_request_fingerprint(kind: &str, term_index: Option<u32>) -> StageFingerprint {
    stage_fingerprint(CompileStageKind::Request, |h| {
        kind.hash(h);
        term_index.hash(h);
    })
}

fn render_cfg_text(module: &ExecutionModule) -> String {
    let mut out = String::new();
    for region in &module.regions {
        let _ = writeln!(out, "region {} entry={}", region.id.0, region.entry.0);
        for block in &region.blocks {
            let params: Vec<String> = block.parameters.iter().map(|p| format!("%{}", p.value.0)).collect();
            let _ = writeln!(out, "  block {} params=[{}]", block.id.0, params.join(","));
            for op in &block.operations {
                let result = match op.result {
                    Some(v) => format!("%{}", v.0),
                    None => "_".to_string(),
                };
                let _ = writeln!(
                    out,
                    "    {} = {} : {}",
                    result,
                    operation_kind_name(&op.kind),
                    value_type_name(&op.result_type)
                );
            }
            let _ = writeln!(out, "    {}", terminator_text(&block.terminator));
        }
    }
    out
}

fn terminator_text(terminator: &Terminator) -> String {
    match terminator {
        Terminator::Return { values } => {
            let ids: Vec<String> = values.iter().map(|v| format!("%{}", v.0)).collect();
            format!("return {}", ids.join(","))
        }
        Terminator::Branch { condition, then_edge, else_edge } => {
            format!(
                "branch %{} then={} else={}",
                condition.0, then_edge.target.0, else_edge.target.0
            )
        }
        Terminator::Switch { discriminant, cases, default } => {
            format!(
                "switch %{} cases={} default={}",
                discriminant.0,
                cases.len(),
                default.target.0
            )
        }
        Terminator::Reject { exit } => format!("reject exit={:?}", exit.map(|e| e.0)),
        Terminator::Yield { values, resume } => {
            format!("yield values={} resume={}", values.len(), resume.target.0)
        }
        Terminator::Unreachable => "unreachable".to_string(),
    }
}

fn operation_kind_name(kind: &OperationKind) -> &'static str {
    match kind {
        OperationKind::LoadInput { .. } => "LoadInput",
        OperationKind::LoadTerm { .. } => "LoadTerm",
        OperationKind::Constant { .. } => "Constant",
        OperationKind::ApplySemanticOperator { .. } => "ApplySemanticOperator",
        OperationKind::ApplyExtensionOperator { .. } => "ApplyExtensionOperator",
        OperationKind::ConstructCollection { .. } => "ConstructCollection",
        OperationKind::Index { .. } => "Index",
        OperationKind::ReadBinding { .. } => "ReadBinding",
        OperationKind::WriteBinding { .. } => "WriteBinding",
        OperationKind::RegisterRuleDispatch { .. } => "RegisterRuleDispatch",
        OperationKind::RegisterCompiledRule { .. } => "RegisterCompiledRule",
        OperationKind::EnterScope { .. } => "EnterScope",
        OperationKind::ExitScope { .. } => "ExitScope",
        OperationKind::CallProvider { .. } => "CallProvider",
        OperationKind::Guard { .. } => "Guard",
        OperationKind::MaterializeValue { .. } => "MaterializeValue",
        OperationKind::PublishResult { .. } => "PublishResult",
    }
}

fn value_type_name(ty: &ExecutionValueType) -> &'static str {
    match ty {
        ExecutionValueType::Unknown => "Unknown",
        ExecutionValueType::Boolean => "Boolean",
        ExecutionValueType::Symbol => "Symbol",
        ExecutionValueType::Term => "Term",
        ExecutionValueType::Value => "Value",
        ExecutionValueType::Result => "Result",
        ExecutionValueType::ProviderPayload => "ProviderPayload",
        ExecutionValueType::Scope => "Scope",
        ExecutionValueType::Unit => "Unit",
    }
}

fn stage_fingerprint(stage: CompileStageKind, body: impl FnOnce(&mut DefaultHasher)) -> StageFingerprint {
    let mut hasher = DefaultHasher::new();
    0x4154_4855_4455_4d50u64.hash(&mut hasher); // "ATHUDUMP"
    core::mem::discriminant(&stage).hash(&mut hasher);
    body(&mut hasher);
    StageFingerprint(hasher.finish())
}

fn stage_diag(stage: CompileStageKind, reason: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::UnsupportedOperation)
        .detail("component", "compiler_dump")
        .detail("stage", format!("{stage:?}"))
        .detail("reason", reason)
}
