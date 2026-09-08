//! Living `04` 薄但真实的编译阶段程序类型。
//!
//! `RequestProgram` / `PlanProgram` 在 fused lowering **之前**产出，是管线真实输入决策。
//! 根与嵌套子请求均 prepare→plan，经 `lower_prepared` 消费阶段载荷。
//! `SemanticProgram` 由 [`elaborate_semantic`] 在 lowering **前**按 Plan 意图 elaboration（独立于 module）。
//! `CfgOutlineProgram` 由 [`elaborate_cfg_outline`] 在 lowering **前**按 Semantic 产出粗粒度 region/entry 意图。
//! `CfgSsaProgram` 仍暂时从已形成的 `ExecutionModule` 物化完整 SSA 文本（诚实边界：尚未独立 CFG formation pass）。
//! [`materialize_semantic`] 仅保留为对照 module 的调试物化，不再作为 staged 主路径。

use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write as _,
    hash::{Hash, Hasher},
};

use crate::{
    api::request::{AthenaRequest, SessionCommand},
    domains::DomainPayloadId,
    execution::ir::{ExecutionModule, ExecutionValueType, ModuleFingerprint, OperationKind, Terminator},
};

/// 编译管线阶段种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CompileStageKind {
    /// Request IR。
    Request,
    /// Plan IR。
    Plan,
    /// Semantic IR。
    Semantic,
    /// CFG SSA IR。
    CfgSsa,
}

/// 阶段级结构指纹（不是 `TermId` 下标）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StageFingerprint(pub u64);

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

/// P0：不可变 Request 程序（薄 canonicalize + 可选 compile 载荷句柄）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq)]
pub struct RequestProgram {
    /// `AthenaRequest::kind_name`。
    pub kind: &'static str,
    /// `Term` 请求时的项下标（仅阶段身份，不作跨 session 身份）。
    pub term_index: Option<u32>,
    /// 控制 / 命令 / Goal 的粗粒度载荷标签（进入 fingerprint）。
    pub payload_tag: Option<&'static str>,
    /// Goal compile 路径：Session 内已 intern 的领域载荷（dump 路径可为空）。
    pub domain_payload: Option<DomainPayloadId>,
    /// Command compile 路径：拥有的会话命令（dump 路径可为空）。
    pub command: Option<SessionCommand>,
    /// Control compile 路径：拥有的控制计划（dump 路径可为空）。
    pub control: Option<crate::api::request::ControlPlan>,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
}

impl RequestProgram {
    /// Owning 复制（含控制计划深拷贝）。
    pub fn owning_copy(&self) -> Self {
        Self {
            kind: self.kind,
            term_index: self.term_index,
            payload_tag: self.payload_tag,
            domain_payload: self.domain_payload,
            command: self.command.clone(),
            control: self.control.as_ref().map(|c| c.owning_copy()),
            fingerprint: self.fingerprint,
        }
    }
}

/// P1：不可变 Plan 程序（薄 planning）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanProgram {
    /// 上游 Request 指纹（阶段链）。
    pub request_fingerprint: StageFingerprint,
    /// 计划意图。
    pub intent: PlanIntent,
    /// 是否需要 provider / domain 载荷。
    pub provider_required: bool,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
}

/// Semantic 操作摘要（薄 Semantic 程序单元）。
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

/// P3：薄 Semantic 程序（Plan 驱动 elaboration · 非 module 反向扫描）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticProgram {
    /// 上游 Plan 指纹（阶段链）。
    pub plan_fingerprint: StageFingerprint,
    /// 计划级操作摘要（lowering 前封闭意图，不是 SSA 回放）。
    pub operations: Vec<SemanticOpSummary>,
    /// 计划 effect 边数量（粗粒度）。
    pub effect_edge_count: usize,
    /// 计划 provider call 数量。
    pub provider_call_count: usize,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
}

/// P4：lowering 前 CFG outline（独立于 module · 粗粒度 region/entry 意图）。
///
/// 完整 SSA 文本 / block 计数仍由 [`materialize_cfg_ssa`] 从 module 物化。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgOutlineProgram {
    /// 上游 Semantic 指纹（阶段链）。
    pub semantic_fingerprint: StageFingerprint,
    /// 计划 region 数（当前 VM 闭集为 1）。
    pub region_count: usize,
    /// 入口 region 下标。
    pub entry_region: u32,
    /// 本阶段 outline 指纹（与物化 [`CfgSsaProgram::fingerprint`] 分立）。
    pub fingerprint: StageFingerprint,
}

/// P4/P5：薄 CFG SSA 程序（当前由 module 物化，非独立 CFG formation）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CfgSsaProgram {
    /// region 数。
    pub region_count: usize,
    /// 全部 block 数。
    pub block_count: usize,
    /// 入口 block 下标（首 region）。
    pub entry_block: u32,
    /// 稳定文本。
    pub text: String,
    /// module 结构指纹。
    pub module_fingerprint: ModuleFingerprint,
    /// 本阶段指纹（绑定文本与 module 指纹）。
    pub fingerprint: StageFingerprint,
}

/// 一次分阶段编译的具名产物（含冻结 module）。
///
/// **不**实现 [`Clone`]。需要副本时分别 `owning_copy` / clone 各阶段产物。
#[derive(Debug, PartialEq)]
pub struct StagedCompile {
    /// P0 Request。
    pub request: RequestProgram,
    /// P1 Plan。
    pub plan: PlanProgram,
    /// P3 Semantic（Plan 驱动 elaboration）。
    pub semantic: SemanticProgram,
    /// P4 CFG outline（Semantic 驱动 · lowering 前）。
    pub cfg_outline: CfgOutlineProgram,
    /// P4/P5 CFG SSA（过渡物化）。
    pub cfg_ssa: CfgSsaProgram,
    /// P9 冻结 module。
    pub module: ExecutionModule,
}

pub(crate) fn stage_fingerprint(stage: CompileStageKind, fill: impl FnOnce(&mut DefaultHasher)) -> StageFingerprint {
    let mut hasher = DefaultHasher::new();
    0x4154_4855_4455_4d50u64.hash(&mut hasher); // "ATHUDUMP"
    core::mem::discriminant(&stage).hash(&mut hasher);
    fill(&mut hasher);
    StageFingerprint(hasher.finish())
}

/// P0：从显式请求 canonicalize 出 Request 程序（无 Session 副作用）。
///
/// 不 intern Goal 载荷、不克隆 Command / Control。compile 路径须再经
/// [`super::ExecutionCompiler`] 的 prepare 步骤填充句柄。
pub fn canonicalize_request(request: &AthenaRequest) -> RequestProgram {
    let term_index = match request {
        AthenaRequest::Term(term) => Some(term.0),
        _ => None,
    };
    let payload_tag = request_payload_tag(request);
    let kind = request.kind_name();
    let fingerprint = request_stage_fingerprint(kind, term_index, payload_tag);
    RequestProgram {
        kind,
        term_index,
        payload_tag,
        domain_payload: None,
        command: None,
        control: None,
        fingerprint,
    }
}

/// Request 阶段指纹（kind · term_index · payload_tag）。
pub fn request_stage_fingerprint(kind: &str, term_index: Option<u32>, payload_tag: Option<&str>) -> StageFingerprint {
    stage_fingerprint(CompileStageKind::Request, |h| {
        kind.hash(h);
        term_index.hash(h);
        payload_tag.hash(h);
    })
}

fn request_payload_tag(request: &AthenaRequest) -> Option<&'static str> {
    match request {
        AthenaRequest::Term(_) => None,
        AthenaRequest::Control(control) => Some(control_plan_tag(control)),
        AthenaRequest::Command(command) => Some(session_command_tag(command)),
        AthenaRequest::Goal(goal) => Some(domain_goal_tag(goal)),
    }
}

fn control_plan_tag(control: &crate::api::request::ControlPlan) -> &'static str {
    use crate::api::request::ControlPlan;
    match control {
        ControlPlan::Sequence { .. } => "Sequence",
        ControlPlan::Branch { .. } => "Branch",
        ControlPlan::Cond { .. } => "Cond",
        ControlPlan::LoopWhile { .. } => "LoopWhile",
        ControlPlan::CountedLoop { .. } => "CountedLoop",
        ControlPlan::Iterate { .. } => "Iterate",
        ControlPlan::Recover { .. } => "Recover",
        ControlPlan::Reject => "Reject",
        ControlPlan::LocalScope { .. } => "LocalScope",
        ControlPlan::LexicalScope { .. } => "LexicalScope",
        ControlPlan::DynamicScope { .. } => "DynamicScope",
        ControlPlan::Index { .. } => "Index",
        ControlPlan::StoreIndex { .. } => "StoreIndex",
        ControlPlan::Match { .. } => "Match",
        ControlPlan::CollectMatches { .. } => "CollectMatches",
        ControlPlan::CollectRejects { .. } => "CollectRejects",
    }
}

fn session_command_tag(command: &SessionCommand) -> &'static str {
    match command {
        SessionCommand::Define { .. } => "Define",
        SessionCommand::DefineMatrix { .. } => "DefineMatrix",
        SessionCommand::RegisterRuleDispatch { .. } => "RegisterRuleDispatch",
        SessionCommand::ClearDefinition { .. } => "ClearDefinition",
    }
}

fn domain_goal_tag(goal: &crate::api::request::DomainGoal) -> &'static str {
    use crate::api::request::DomainGoal;
    use crate::domains::DomainRequest;
    match goal {
        DomainGoal::Dispatch(DomainRequest::Calculus(_)) => "Calculus",
        DomainGoal::Dispatch(DomainRequest::NumberTheory(_)) => "NumberTheory",
        DomainGoal::Dispatch(DomainRequest::Polynomial(_)) => "Polynomial",
        DomainGoal::Dispatch(DomainRequest::GroupTheory(_)) => "GroupTheory",
        DomainGoal::Dispatch(DomainRequest::FieldTheory(_)) => "FieldTheory",
        DomainGoal::Dispatch(DomainRequest::GaloisTheory(_)) => "GaloisTheory",
        DomainGoal::Dispatch(DomainRequest::GraphTheory(_)) => "GraphTheory",
        DomainGoal::Dispatch(DomainRequest::LinearAlgebra(_)) => "LinearAlgebra",
        DomainGoal::Dispatch(DomainRequest::Optimization(_)) => "Optimization",
        DomainGoal::Dispatch(DomainRequest::Solve(_)) => "Solve",
    }
}

/// P1：由 Request 程序产出 Plan 程序（不读 module、不 emit 指令）。
pub fn plan_from_request(request: &RequestProgram) -> PlanProgram {
    let (intent, provider_required) = match request.kind {
        "Term" => (PlanIntent::EvaluateTerm, false),
        "Control" => (PlanIntent::RunControl, false),
        "Command" => (PlanIntent::SessionCommand, false),
        "Goal" => (PlanIntent::DomainProvider, true),
        _ => (PlanIntent::EvaluateTerm, false),
    };
    // 指纹纳入上游 Request，形成真实阶段链（相对旧 dump_plan 有意演进）。
    let fingerprint = stage_fingerprint(CompileStageKind::Plan, |h| {
        request.fingerprint.0.hash(h);
        core::mem::discriminant(&intent).hash(h);
        provider_required.hash(h);
    });
    PlanProgram { request_fingerprint: request.fingerprint, intent, provider_required, fingerprint }
}

fn planned_op(kind: &'static str, result_type: &'static str) -> SemanticOpSummary {
    SemanticOpSummary { kind, result_type, effect_in: None, effect_out: None }
}

/// P3：在 lowering **前**由 Request/Plan elaboration 出 Semantic 程序（不读 module）。
pub fn elaborate_semantic(request: &RequestProgram, plan: &PlanProgram) -> SemanticProgram {
    let operations = match plan.intent {
        PlanIntent::EvaluateTerm => vec![planned_op("TermElaboration", "Term")],
        PlanIntent::RunControl => {
            let tag = request.payload_tag.unwrap_or("Control");
            vec![planned_op(tag, "Unit")]
        }
        PlanIntent::SessionCommand => match request.payload_tag.unwrap_or("Command") {
            "Define" | "DefineMatrix" | "ClearDefinition" => vec![planned_op("WriteBinding", "Unit")],
            "RegisterRuleDispatch" => vec![planned_op("RegisterCompiledRule", "Unit")],
            other => vec![planned_op(other, "Unit")],
        },
        PlanIntent::DomainProvider => vec![planned_op("CallProvider", "Result")],
    };
    let effect_edge_count = operations
        .iter()
        .filter(|op| matches!(op.kind, "WriteBinding" | "RegisterCompiledRule" | "CallProvider"))
        .count();
    let provider_call_count = usize::from(plan.provider_required);
    let fingerprint = stage_fingerprint(CompileStageKind::Semantic, |h| {
        plan.fingerprint.0.hash(h);
        operations.len().hash(h);
        for op in &operations {
            op.kind.hash(h);
            op.result_type.hash(h);
        }
        effect_edge_count.hash(h);
        provider_call_count.hash(h);
    });
    SemanticProgram {
        plan_fingerprint: plan.fingerprint,
        operations,
        effect_edge_count,
        provider_call_count,
        fingerprint,
    }
}

/// P4：在 lowering **前**由 Semantic 产出 CFG outline（不读 module、不 emit SSA）。
pub fn elaborate_cfg_outline(semantic: &SemanticProgram) -> CfgOutlineProgram {
    // 当前 verify/codegen 闭集仍是单 region。多 region 属独立 CFG formation 后续债。
    let region_count = 1usize;
    let entry_region = 0u32;
    let fingerprint = stage_fingerprint(CompileStageKind::CfgSsa, |h| {
        1u8.hash(h); // outline marker（与物化 CfgSsa fingerprint 分立）
        semantic.fingerprint.0.hash(h);
        region_count.hash(h);
        entry_region.hash(h);
    });
    CfgOutlineProgram {
        semantic_fingerprint: semantic.fingerprint,
        region_count,
        entry_region,
        fingerprint,
    }
}

/// 校验 outline 与物化 CFG 的粗粒度一致（region 数 / 入口 region）。
pub fn verify_cfg_outline(outline: &CfgOutlineProgram, cfg: &CfgSsaProgram) -> Result<(), &'static str> {
    if outline.region_count != cfg.region_count {
        return Err("cfg_outline_region_count_mismatch");
    }
    if outline.entry_region != 0 {
        return Err("cfg_outline_entry_region_unsupported");
    }
    Ok(())
}

/// 调试：从已形成 module 物化 Semantic 视图（非 staged 主路径）。
pub fn materialize_semantic(module: &ExecutionModule) -> SemanticProgram {
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
        0u8.hash(h); // realized-from-module marker
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
    SemanticProgram {
        plan_fingerprint: StageFingerprint(0),
        operations,
        effect_edge_count,
        provider_call_count,
        fingerprint,
    }
}

/// 过渡：从已形成 module 物化 CFG SSA 程序。
pub fn materialize_cfg_ssa(module: &ExecutionModule) -> CfgSsaProgram {
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
    CfgSsaProgram { region_count, block_count, entry_block, text, module_fingerprint, fingerprint }
}

pub(crate) fn render_cfg_text(module: &ExecutionModule) -> String {
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
                let _ = writeln!(out, "    {} = {} : {}", result, operation_kind_name(&op.kind), value_type_name(&op.result_type));
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
            format!("branch %{} then={} else={}", condition.0, then_edge.target.0, else_edge.target.0)
        }
        Terminator::Switch { discriminant, cases, default } => {
            format!("switch %{} cases={} default={}", discriminant.0, cases.len(), default.target.0)
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
        OperationKind::StoreIndex { .. } => "StoreIndex",
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
