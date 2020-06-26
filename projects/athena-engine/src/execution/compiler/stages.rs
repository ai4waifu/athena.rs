//! Living `04` 薄但真实的编译阶段程序类型。
//!
//! `RequestProgram` / `PlanProgram` 在 fused lowering **之前**产出，是管线真实输入决策。
//! `SemanticProgram` / `CfgSsaProgram` 仍暂时从已形成的 `ExecutionModule` 物化
//! （诚实边界：尚未独立 Semantic elaboration / CFG formation pass），但它们是具名阶段产物，
//! 不再只是 `observe_compile` 的事后视图别名。

use std::{
    collections::hash_map::DefaultHasher,
    fmt::Write as _,
    hash::{Hash, Hasher},
};

use crate::{
    api::request::AthenaRequest,
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

/// P0：不可变 Request 程序（薄 canonicalize）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestProgram {
    /// `AthenaRequest::kind_name`。
    pub kind: &'static str,
    /// `Term` 请求时的项下标（仅阶段身份，不作跨 session 身份）。
    pub term_index: Option<u32>,
    /// 本阶段指纹。
    pub fingerprint: StageFingerprint,
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

/// P3：薄 Semantic 程序（当前由 module 物化，非独立 elaboration）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticProgram {
    /// 按块遍历顺序的操作摘要。
    pub operations: Vec<SemanticOpSummary>,
    /// effect 边数量。
    pub effect_edge_count: usize,
    /// provider call 描述符数量。
    pub provider_call_count: usize,
    /// 本阶段指纹。
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
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedCompile {
    /// P0 Request。
    pub request: RequestProgram,
    /// P1 Plan。
    pub plan: PlanProgram,
    /// P3 Semantic（过渡物化）。
    pub semantic: SemanticProgram,
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

/// P0：从显式请求 canonicalize 出 Request 程序。
pub fn canonicalize_request(request: &AthenaRequest) -> RequestProgram {
    let term_index = match request {
        AthenaRequest::Term(term) => Some(term.0),
        _ => None,
    };
    let kind = request.kind_name();
    let fingerprint = stage_fingerprint(CompileStageKind::Request, |h| {
        kind.hash(h);
        term_index.hash(h);
    });
    RequestProgram { kind, term_index, fingerprint }
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

/// 过渡：从已形成 module 物化 Semantic 程序。
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
    SemanticProgram { operations, effect_edge_count, provider_call_count, fingerprint }
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
