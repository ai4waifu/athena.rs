//! [`ResultId`] → [`ComputationResult`] 存储。

use std::collections::BTreeMap;

use athena_types::{ComputationStatus, Condition, Diagnostic, ResultId, TermId, ValueId};

use super::CoverageStatus;

/// 结果关联的 capability provider（结果层身份，后续可与 M-Graph provider 对齐）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResultProviderId(pub u32);

impl ResultProviderId {
    /// 微积分。
    pub const CALCULUS: Self = Self(1);
    /// 数论。
    pub const NUMBER_THEORY: Self = Self(2);
    /// 多项式。
    pub const POLYNOMIAL: Self = Self(3);
    /// 群论。
    pub const GROUP: Self = Self(4);
    /// 域论。
    pub const FIELD: Self = Self(5);
    /// 伽罗瓦。
    pub const GALOIS: Self = Self(6);
    /// 图论。
    pub const GRAPH_THEORY: Self = Self(7);
    /// 线性代数。
    pub const LINEAR_ALGEBRA: Self = Self(8);
    /// 优化。
    pub const OPTIMIZATION: Self = Self(9);

    /// 结果层 provider 合同版本（Frontier resume 兼容检查）。
    ///
    /// Bootstrap：全域统一从 `1` 起。破坏性变更时递增，旧 `ResumeToken` 不得静默继续。
    pub const CONTRACT_VERSION: u32 = 1;

    /// 附上当前合同版本。
    pub const fn stamped(self) -> ResultProviderStamp {
        ResultProviderStamp { id: self, version: Self::CONTRACT_VERSION }
    }
}

/// Provider 身份 + 合同版本（结果 / Frontier 共用戳）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResultProviderStamp {
    /// Provider 身份。
    pub id: ResultProviderId,
    /// 合同版本（与 [`ResultProviderId::CONTRACT_VERSION`] 对齐时方可 resume）。
    pub version: u32,
}

impl ResultProviderStamp {
    /// 身份相同且版本精确匹配则兼容（bootstrap：禁止静默跨版本恢复）。
    pub const fn compatible_with(self, other: Self) -> bool {
        self.id.0 == other.id.0 && self.version == other.version
    }

    /// 是否匹配当前进程的合同版本。
    pub const fn matches_current_contract(self) -> bool {
        self.compatible_with(self.id.stamped())
    }
}

/// 结果证据引用（typed evidence store 落地前的结果层句柄）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResultEvidence {
    /// 内核可重放摘要（展示用；不得单独充当机器证明）。
    TrustedKernelSummary {
        /// provider。
        provider: ResultProviderId,
        /// 人类可读摘要。
        summary: String,
    },
}

/// 结果来源审计。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResultProvenance {
    /// 请求种类名（`Term` / `DomainGoal` / `Command` / …）。
    pub request_kind: &'static str,
    /// `ExecutionIR` provider capability fingerprint（无 provider 边时为 `None`）。
    pub capability_fingerprint: Option<u64>,
}

impl ResultProvenance {
    /// 无 provider capability 边的由来。
    pub fn kind(request_kind: &'static str) -> Self {
        Self { request_kind, capability_fingerprint: None }
    }

    /// `CallProvider` 边的由来。
    pub fn call_provider(capability_fingerprint: u64) -> Self {
        Self { request_kind: "CallProvider", capability_fingerprint: Some(capability_fingerprint) }
    }
}

/// 一次计算的可观察结果。
#[derive(Debug, PartialEq)]
pub struct ComputationResult {
    /// 计算 / 验证状态。
    pub status: ComputationStatus,
    /// 覆盖范围（完整 / 部分 / 未支持等）。
    pub coverage: CoverageStatus,
    /// 运行时值（若有）。
    pub value: Option<ValueId>,
    /// 符号项投影（若有）。不得单独冒充「成功值」。
    pub symbolic_term: Option<TermId>,
    /// 结果成立所需条件。
    pub conditions: Vec<Condition>,
    /// 结构化诊断。
    pub diagnostics: Vec<Diagnostic>,
    /// 证据引用（typed store 接入前可为空）。
    pub evidence: Vec<ResultEvidence>,
    /// 产出 provider（含合同版本 · ）。
    pub provider: Option<ResultProviderStamp>,
    /// 来源审计。
    pub provenance: Option<ResultProvenance>,
}

impl ComputationResult {
    /// 构造带状态的空结果骨架。
    pub fn with_status(status: ComputationStatus, coverage: CoverageStatus) -> Self {
        Self {
            status,
            coverage,
            value: None,
            symbolic_term: None,
            conditions: Vec::new(),
            diagnostics: Vec::new(),
            evidence: Vec::new(),
            provider: None,
            provenance: None,
        }
    }

    /// 附加诊断。
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
        self
    }

    /// 附加条件。
    pub fn with_condition(mut self, condition: Condition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// 附加证据。
    pub fn with_evidence(mut self, evidence: ResultEvidence) -> Self {
        self.evidence.push(evidence);
        self
    }

    /// 附加 provider（自动盖上当前合同版本戳）。
    pub fn with_provider(mut self, provider: ResultProviderId) -> Self {
        self.provider = Some(provider.stamped());
        self
    }

    /// 附加已显式版本化的 provider 戳。
    pub fn with_provider_stamp(mut self, stamp: ResultProviderStamp) -> Self {
        self.provider = Some(stamp);
        self
    }

    /// 附加来源。
    pub fn with_provenance(mut self, provenance: ResultProvenance) -> Self {
        self.provenance = Some(provenance);
        self
    }

    /// 附加可选运行时值。
    pub fn with_value(mut self, value: ValueId) -> Self {
        self.value = Some(value);
        self
    }

    /// 附加可选符号项投影。
    pub fn with_symbolic_term(mut self, term: TermId) -> Self {
        self.symbolic_term = Some(term);
        self
    }
}

/// [`ResultId`] 所有者：持有真实 [`ComputationResult`] 载荷。
#[derive(Debug, Default, PartialEq)]
pub struct ResultStore {
    next: u32,
    results: BTreeMap<ResultId, ComputationResult>,
}

impl ResultStore {
    /// 空存储。
    pub fn new() -> Self {
        Self::default()
    }

    /// 插入计算结果并返回新身份。
    pub fn insert(&mut self, result: ComputationResult) -> ResultId {
        let id = ResultId(self.next);
        self.next = self.next.saturating_add(1);
        self.results.insert(id, result);
        id
    }

    /// 读取载荷。
    pub fn get(&self, id: ResultId) -> Option<&ComputationResult> {
        self.results.get(&id)
    }

    /// 是否已分配。
    pub fn contains(&self, id: ResultId) -> bool {
        self.results.contains_key(&id)
    }

    /// 已分配结果条数（不是序列长度）。
    pub fn count(&self) -> usize {
        self.results.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}
