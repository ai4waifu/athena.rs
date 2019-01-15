//! 纯 Rust 内核的调用方自有 limb 缓冲与 scratch 工作区。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::execution_budget::ExecutionBudget;

use super::limb_kernel::{effective_len, normalize_trim};

/// 可增长小端 limb 缓冲（工作中存储可含尾随零）。
#[derive(Debug, Clone, Default)]
pub(crate) struct LimbBuffer {
    limbs: Vec<u64>,
}

/// 跨内核原语复用的 scratch 池。
#[derive(Debug, Default)]
pub(crate) struct ScratchWorkspace {
    pool: Vec<LimbBuffer>,
}

impl LimbBuffer {
    /// 含规范零 `[0]` 的空缓冲。
    pub(crate) fn zero() -> Self {
        Self { limbs: vec![0] }
    }

    /// 在预算下分配至少 `limbs` 项容量。
    pub(crate) fn with_capacity(limbs: usize, budget: &ExecutionBudget) -> Result<Self> {
        budget.check_limbs(limbs)?;
        Ok(Self { limbs: Vec::with_capacity(limbs) })
    }

    /// 在预算下导入规范 limb。
    pub(crate) fn from_canonical(v: &[u64], budget: &ExecutionBudget) -> Result<Self> {
        let el = effective_len(v);
        budget.check_limbs(el)?;
        let mut limbs = Vec::with_capacity(el);
        limbs.extend_from_slice(&v[..el]);
        if limbs.is_empty() {
            limbs.push(0);
        }
        Ok(Self { limbs })
    }

    /// 规范小端切片（已修剪，至少一个 limb）。
    pub(crate) fn as_canonical(&self) -> &[u64] {
        let el = effective_len(&self.limbs);
        &self.limbs[..el]
    }

    /// 逻辑规范 limb 数。
    pub(crate) fn canonical_len(&self) -> usize {
        effective_len(&self.limbs)
    }

    /// 原始存储长度（工作中可超过规范长度）。
    pub(crate) fn storage_len(&self) -> usize {
        self.limbs.len()
    }

    /// 确保存储可容纳 `limbs` 项（不必已规范）。
    pub(crate) fn ensure_storage(&mut self, limbs: usize, budget: &ExecutionBudget) -> Result<()> {
        budget.check_limbs(limbs)?;
        if self.limbs.len() < limbs {
            self.limbs.resize(limbs, 0);
        }
        Ok(())
    }

    /// 用规范 limb 替换内容。
    pub(crate) fn set_canonical(&mut self, v: Vec<u64>, budget: &ExecutionBudget) -> Result<()> {
        let trimmed = normalize_trim(v);
        budget.check_limbs(trimmed.len())?;
        self.limbs = trimmed;
        Ok(())
    }

    /// 规范 limb 向量（已修剪）。
    pub(crate) fn into_canonical_vec(self) -> Vec<u64> {
        normalize_trim(self.limbs)
    }

    /// 供内核写入的可变存储切片（`len` 项）。
    pub(crate) fn storage_mut(&mut self, len: usize, budget: &ExecutionBudget) -> Result<&mut [u64]> {
        self.ensure_storage(len, budget)?;
        Ok(&mut self.limbs[..len])
    }

    /// 就地修剪尾随零至规范形。
    pub(crate) fn trim_canonical(&mut self) {
        self.limbs = normalize_trim(self.limbs.clone());
    }
}

impl ScratchWorkspace {
    /// 借用至少 `capacity` 个 limb 的 scratch 缓冲。
    pub(crate) fn buffer(&mut self, capacity: usize, budget: &ExecutionBudget) -> Result<&mut LimbBuffer> {
        budget.check_limbs(capacity)?;
        if self.pool.is_empty() {
            self.pool.push(LimbBuffer::with_capacity(capacity, budget)?);
        }
        let buf = &mut self.pool[0];
        buf.ensure_storage(capacity, budget)?;
        Ok(buf)
    }

    /// 在顶层运算之间清空 scratch 缓冲。
    pub(crate) fn clear(&mut self) {
        for b in &mut self.pool {
            b.limbs.clear();
            b.limbs.push(0);
        }
    }
}

pub fn kernel_err(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "numeric").detail("operation", op)
}
