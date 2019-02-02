//! 纯 Rust 内核的调用方自有 limb 缓冲与 scratch 工作区。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::policy::execution_budget::ExecutionBudget;

use crate::kernel::pure_rust::limb_kernel::effective_len;

/// 可增长小端 limb 缓冲（工作中存储可含尾随零）。
#[derive(Debug, Clone, Default)]
pub(crate) struct LimbBuffer {
    limbs: Vec<u64>,
}

/// 顶层一次分配的连续 limb scratch（bump；经 [`crate::NumericContext`] 统一挂钩）。
#[derive(Debug, Default)]
pub struct ScratchWorkspace {
    arena: Vec<u64>,
    cursor: usize,
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
        budget.check_limbs(el.max(1))?;
        let mut limbs = Vec::with_capacity(el.max(1));
        if el == 0 {
            limbs.push(0);
        }
        else {
            limbs.extend_from_slice(&v[..el]);
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

    /// 将规范 limb 切片写入本缓冲（复用容量，不接收新 `Vec`）。
    pub(crate) fn copy_canonical(&mut self, v: &[u64], budget: &ExecutionBudget) -> Result<()> {
        let el = effective_len(v);
        let n = el.max(1);
        self.ensure_storage(n, budget)?;
        if el == 0 {
            self.limbs[0] = 0;
            self.limbs.truncate(1);
        }
        else {
            self.limbs[..el].copy_from_slice(&v[..el]);
            self.limbs.truncate(el);
        }
        Ok(())
    }

    /// 写入规范零。
    pub(crate) fn set_zero(&mut self, budget: &ExecutionBudget) -> Result<()> {
        self.ensure_storage(1, budget)?;
        self.limbs[0] = 0;
        self.limbs.truncate(1);
        Ok(())
    }

    /// 规范 limb 向量（已修剪）。
    pub(crate) fn into_canonical_vec(self) -> Vec<u64> {
        let mut limbs = self.limbs;
        while limbs.len() > 1 && limbs.last() == Some(&0) {
            limbs.pop();
        }
        if limbs.is_empty() {
            limbs.push(0);
        }
        limbs
    }

    /// 供内核写入的可变存储切片（`len` 项，调用方负责清零/填充）。
    pub(crate) fn storage_mut(&mut self, len: usize, budget: &ExecutionBudget) -> Result<&mut [u64]> {
        self.ensure_storage(len, budget)?;
        if self.limbs.len() > len {
            self.limbs.truncate(len);
        }
        Ok(&mut self.limbs[..len])
    }

    /// 就地修剪尾随零至规范形（不重新分配，仅 `pop`）。
    pub(crate) fn trim_canonical(&mut self) {
        while self.limbs.len() > 1 && self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.limbs.push(0);
        }
    }
}

impl ScratchWorkspace {
    /// 空 scratch。
    pub fn new() -> Self {
        Self::default()
    }

    /// 确保竞技场至少 `limbs` 项，并重置 bump 游标。
    pub fn ensure(&mut self, limbs: usize, budget: &ExecutionBudget) -> Result<()> {
        budget.check_limbs(limbs.max(1))?;
        if self.arena.capacity() < limbs {
            self.arena = Vec::with_capacity(limbs);
        }
        if self.arena.len() < limbs {
            self.arena.resize(limbs, 0);
        }
        self.cursor = 0;
        Ok(())
    }

    /// 从竞技场 bump `n` 个已清零 limb；不足时返回诊断（调用方须先 `ensure`）。
    pub fn alloc(&mut self, n: usize) -> Result<&mut [u64]> {
        let start = self.cursor;
        let end = start.checked_add(n).ok_or_else(|| kernel_err("scratch_overflow"))?;
        if end > self.arena.len() {
            return Err(kernel_err("scratch_underrun"));
        }
        self.cursor = end;
        let s = &mut self.arena[start..end];
        s.fill(0);
        Ok(s)
    }

    /// 当前 bump 标记（供递归子作用域回滚）。
    pub fn mark(&self) -> usize {
        self.cursor
    }

    /// 回滚到先前标记。
    pub fn rewind(&mut self, mark: usize) {
        debug_assert!(mark <= self.cursor);
        self.cursor = mark;
    }

    /// 连续 scratch 存储（整片，含未 bump 区域）。
    pub(crate) fn as_mut_slice(&mut self) -> &mut [u64] {
        &mut self.arena
    }

    /// 在顶层运算之间清空逻辑内容（保留容量，重置游标）。
    pub fn clear(&mut self) {
        self.arena.fill(0);
        self.cursor = 0;
    }
}

pub fn kernel_err(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::DomainError).detail("domain", "numeric").detail("operation", op)
}
