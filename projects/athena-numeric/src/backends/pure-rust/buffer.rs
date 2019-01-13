//! Caller-owned limb buffers and scratch workspace for the pure-Rust kernel.

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::execution_budget::ExecutionBudget;

use super::limb_kernel::{effective_len, normalize_trim};

/// Growable little-endian limb buffer (storage may include trailing zeros during work).
#[derive(Debug, Clone, Default)]
pub(crate) struct LimbBuffer {
    limbs: Vec<u64>,
}

/// Scratch pool reused across kernel primitives.
#[derive(Debug, Default)]
pub(crate) struct ScratchWorkspace {
    pool: Vec<LimbBuffer>,
}

impl LimbBuffer {
    /// Empty buffer with canonical zero `[0]`.
    pub(crate) fn zero() -> Self {
        Self { limbs: vec![0] }
    }

    /// Allocate capacity for at least `limbs` entries under budget.
    pub(crate) fn with_capacity(limbs: usize, budget: &ExecutionBudget) -> Result<Self> {
        budget.check_limbs(limbs)?;
        Ok(Self { limbs: Vec::with_capacity(limbs) })
    }

    /// Import canonical limbs under budget.
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

    /// Canonical little-endian slice (trimmed, at least one limb).
    pub(crate) fn as_canonical(&self) -> &[u64] {
        let el = effective_len(&self.limbs);
        &self.limbs[..el]
    }

    /// Logical canonical limb count.
    pub(crate) fn canonical_len(&self) -> usize {
        effective_len(&self.limbs)
    }

    /// Raw storage length (may exceed canonical length during work).
    pub(crate) fn storage_len(&self) -> usize {
        self.limbs.len()
    }

    /// Ensure storage can hold `limbs` entries (not necessarily canonical).
    pub(crate) fn ensure_storage(&mut self, limbs: usize, budget: &ExecutionBudget) -> Result<()> {
        budget.check_limbs(limbs)?;
        if self.limbs.len() < limbs {
            self.limbs.resize(limbs, 0);
        }
        Ok(())
    }

    /// Replace contents with canonical limbs.
    pub(crate) fn set_canonical(&mut self, v: Vec<u64>, budget: &ExecutionBudget) -> Result<()> {
        let trimmed = normalize_trim(v);
        budget.check_limbs(trimmed.len())?;
        self.limbs = trimmed;
        Ok(())
    }

    /// Canonical limb vector (trimmed).
    pub(crate) fn into_canonical_vec(self) -> Vec<u64> {
        normalize_trim(self.limbs)
    }

    /// Mutable storage slice for kernel writes (`len` entries).
    pub(crate) fn storage_mut(&mut self, len: usize, budget: &ExecutionBudget) -> Result<&mut [u64]> {
        self.ensure_storage(len, budget)?;
        Ok(&mut self.limbs[..len])
    }

    /// Trim trailing zeros in place to canonical form.
    pub(crate) fn trim_canonical(&mut self) {
        self.limbs = normalize_trim(self.limbs.clone());
    }
}

impl ScratchWorkspace {
    /// Borrow a scratch buffer with at least `capacity` limbs.
    pub(crate) fn buffer(&mut self, capacity: usize, budget: &ExecutionBudget) -> Result<&mut LimbBuffer> {
        budget.check_limbs(capacity)?;
        if self.pool.is_empty() {
            self.pool.push(LimbBuffer::with_capacity(capacity, budget)?);
        }
        let buf = &mut self.pool[0];
        buf.ensure_storage(capacity, budget)?;
        Ok(buf)
    }

    /// Clear scratch buffers between top-level operations.
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
