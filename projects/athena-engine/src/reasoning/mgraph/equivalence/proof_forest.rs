//! 已接纳等式的证明森林（引导实现）。
//!
//! 记录经 AdmissionGate 后两项相等的 *原因*。有别于
//! 作用域局部 E-Graph 候选合并，也有别于操作层 [`ExactUnionFind`]。

use athena_types::TermId;

/// 森林中一条有理据的等式边。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofEdge {
    /// Left term.
    pub left: TermId,
    /// Right term.
    pub right: TermId,
    /// 不透明步骤种类（稍后由验证器填充）。
    pub step_kind: ProofStepKind,
}

/// 引导用的封闭步骤分类（稍后用证书扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofStepKind {
    /// 直接接纳的等式（结构 / harness / 通用）。
    AdmittedEquality,
    /// 相同头部下的同余（ExactUF 应用同余）。
    Congruence,
    /// 带类型重写回放（`match_pattern` + `substitute`）。
    TypedRewrite,
    /// 传递性步骤。
    Transitivity,
}

/// 等式理据森林（仅追加的引导实现）。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProofForest {
    edges: Vec<ProofEdge>,
}

impl ProofForest {
    /// 空森林。
    pub fn new() -> Self {
        Self::default()
    }

    /// 记录一条有理据等式（本身不接纳 M-Graph 事实）。
    pub fn record(&mut self, left: TermId, right: TermId, step_kind: ProofStepKind) {
        self.edges.push(ProofEdge { left, right, step_kind });
    }

    /// Edge count.
    pub fn len(&self) -> usize {
        self.edges.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// 迭代边。
    pub fn edges(&self) -> &[ProofEdge] {
        &self.edges
    }
}
