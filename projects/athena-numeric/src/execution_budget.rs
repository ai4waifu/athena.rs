//! Allocation and growth budgets for numeric kernel execution.

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::backends::{NumericBackend, NumericBackendLimits, PureRustBackend};

/// Execution budget wired from backend limits or Session policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionBudget {
    max_limbs: Option<u32>,
    max_significand_bits: Option<u32>,
    max_wire_payload_bytes: Option<u32>,
}

impl ExecutionBudget {
    /// No limb or payload caps (development / tests).
    pub fn unlimited() -> Self {
        Self { max_limbs: None, max_significand_bits: None, max_wire_payload_bytes: None }
    }

    /// Build from a static backend contract.
    pub fn from_limits(limits: &NumericBackendLimits) -> Self {
        Self {
            max_limbs: limits.max_limbs,
            max_significand_bits: limits.max_significand_bits,
            max_wire_payload_bytes: limits.max_wire_payload_bytes,
        }
    }

    /// Maximum canonical limb count, if bounded.
    pub fn max_limbs(&self) -> Option<u32> {
        self.max_limbs
    }

    /// Maximum significand bit width for arbitrary floats.
    pub fn max_significand_bits(&self) -> Option<u32> {
        self.max_significand_bits
    }

    /// Maximum wire payload bytes for decode.
    pub fn max_wire_payload_bytes(&self) -> Option<u32> {
        self.max_wire_payload_bytes
    }

    /// Reject a buffer that would hold `limbs` canonical limbs.
    pub fn check_limbs(&self, limbs: usize) -> Result<()> {
        if let Some(max) = self.max_limbs {
            if limbs > max as usize {
                return Err(resource_limit("limbs", limbs, max));
            }
        }
        Ok(())
    }

    /// Reject a significand wider than policy.
    pub fn check_significand_bits(&self, bits: u64) -> Result<()> {
        if let Some(max) = self.max_significand_bits {
            if bits > u64::from(max) {
                return Err(resource_limit("significand_bits", bits as usize, max));
            }
        }
        Ok(())
    }

    /// Reject wire payloads larger than policy.
    pub fn check_wire_bytes(&self, bytes: usize) -> Result<()> {
        if let Some(max) = self.max_wire_payload_bytes {
            if bytes > max as usize {
                return Err(resource_limit("wire_bytes", bytes, max));
            }
        }
        Ok(())
    }

    /// Estimate and check output limbs for addition.
    pub fn check_add(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let out = a_limbs.max(b_limbs) + 1;
        self.check_limbs(out)
    }

    /// Estimate and check output limbs for multiplication.
    pub fn check_mul(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let out = a_limbs + b_limbs;
        self.check_limbs(out)
    }

    /// Estimate and check scratch for Karatsuba multiply (conservative).
    pub fn check_mul_scratch(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let n = a_limbs.max(b_limbs);
        self.check_limbs(n * 4 + a_limbs + b_limbs)
    }

    /// Estimate and check quotient buffer for division.
    pub fn check_div(&self, u_limbs: usize, v_limbs: usize) -> Result<()> {
        let q = if v_limbs == 0 { u_limbs + 1 } else { u_limbs.saturating_sub(v_limbs) + 1 };
        self.check_limbs(q.max(u_limbs) + v_limbs + 2)
    }
}

/// Numeric execution context: budget + backend selection hook.
#[derive(Debug, Clone, Copy)]
pub struct NumericContext {
    budget: ExecutionBudget,
}

impl NumericContext {
    /// Pure-Rust default limits from [`crate::backends::PureRustBackend`].
    pub fn pure_rust_default() -> Self {
        Self { budget: ExecutionBudget::from_limits(&NumericBackend::contract(&PureRustBackend::default()).limits) }
    }

    /// Unlimited budget.
    pub fn unlimited() -> Self {
        Self { budget: ExecutionBudget::unlimited() }
    }

    /// Active budget.
    pub fn budget(&self) -> &ExecutionBudget {
        &self.budget
    }
}

fn resource_limit(kind: &str, got: usize, max: u32) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericResourceLimit)
        .detail("domain", "numeric")
        .detail("kind", kind)
        .detail("got", got.to_string())
        .detail("max", max.to_string())
}
