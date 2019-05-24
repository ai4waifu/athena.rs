//! [`ResultId`] → [`ComputationResult`] 存储（Living `26`）。

use std::collections::BTreeMap;

use athena_types::{ComputationStatus, Diagnostic, ResultId, TermId, ValueId};

use super::CoverageStatus;

/// 一次计算的可观察结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputationResult {
    /// 计算 / 验证状态。
    pub status: ComputationStatus,
    /// 覆盖范围（完整 / 部分 / 未支持等）。
    pub coverage: CoverageStatus,
    /// 运行时值（若有）。
    pub value: Option<ValueId>,
    /// 符号项投影（若有）。不得单独冒充「成功值」。
    pub symbolic_term: Option<TermId>,
    /// 结构化诊断。
    pub diagnostics: Vec<Diagnostic>,
}

impl ComputationResult {
    /// 构造带状态的空结果骨架。
    pub fn with_status(status: ComputationStatus, coverage: CoverageStatus) -> Self {
        Self { status, coverage, value: None, symbolic_term: None, diagnostics: Vec::new() }
    }

    /// 附加诊断。
    pub fn with_diagnostic(mut self, diagnostic: Diagnostic) -> Self {
        self.diagnostics.push(diagnostic);
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
#[derive(Debug, Clone, Default, PartialEq, Eq)]
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

    /// 已分配数量。
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}
