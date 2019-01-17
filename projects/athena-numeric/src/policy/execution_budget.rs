//! 数值内核执行的分配与增长预算。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::backend::{NumericBackend, NumericBackendLimits, PureRustBackend};

/// 由 backend 上限或 Session 策略接入的执行预算。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionBudget {
    max_limbs: Option<u32>,
    max_significand_bits: Option<u32>,
    max_wire_payload_bytes: Option<u32>,
}

impl ExecutionBudget {
    /// 无 limb / 载荷上限（开发 / 测试）。
    pub fn unlimited() -> Self {
        Self { max_limbs: None, max_significand_bits: None, max_wire_payload_bytes: None }
    }

    /// 由静态 backend 合同构造。
    pub fn from_limits(limits: &NumericBackendLimits) -> Self {
        Self {
            max_limbs: limits.max_limbs,
            max_significand_bits: limits.max_significand_bits,
            max_wire_payload_bytes: limits.max_wire_payload_bytes,
        }
    }

    /// 规范 limb 数上限（若有界）。
    pub fn max_limbs(&self) -> Option<u32> {
        self.max_limbs
    }

    /// 任意精度浮点最大尾数位宽。
    pub fn max_significand_bits(&self) -> Option<u32> {
        self.max_significand_bits
    }

    /// 解码用 wire 载荷最大字节数。
    pub fn max_wire_payload_bytes(&self) -> Option<u32> {
        self.max_wire_payload_bytes
    }

    /// 拒绝将容纳 `limbs` 个规范 limb 的缓冲。
    pub fn check_limbs(&self, limbs: usize) -> Result<()> {
        if let Some(max) = self.max_limbs {
            if limbs > max as usize {
                return Err(resource_limit("limbs", limbs, max));
            }
        }
        Ok(())
    }

    /// 拒绝宽于策略的尾数。
    pub fn check_significand_bits(&self, bits: u64) -> Result<()> {
        if let Some(max) = self.max_significand_bits {
            if bits > u64::from(max) {
                return Err(resource_limit("significand_bits", bits as usize, max));
            }
        }
        Ok(())
    }

    /// 拒绝大于策略的 wire 载荷。
    pub fn check_wire_bytes(&self, bytes: usize) -> Result<()> {
        if let Some(max) = self.max_wire_payload_bytes {
            if bytes > max as usize {
                return Err(resource_limit("wire_bytes", bytes, max));
            }
        }
        Ok(())
    }

    /// 估算并检查加法输出 limb 数。
    pub fn check_add(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let out = a_limbs.max(b_limbs) + 1;
        self.check_limbs(out)
    }

    /// 估算并检查乘法输出 limb 数。
    pub fn check_mul(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let out = a_limbs + b_limbs;
        self.check_limbs(out)
    }

    /// 估算并检查 Karatsuba 乘法 scratch（保守上界，与内核逐层公式同阶）。
    pub fn check_mul_scratch(&self, a_limbs: usize, b_limbs: usize) -> Result<()> {
        let n = a_limbs.max(b_limbs);
        if n < 32 {
            return self.check_limbs(a_limbs + b_limbs);
        }
        // 与 `karatsuba_scratch_limbs` 同构：每层 z0+z2+asum+bsum+z1
        let mut m = n.next_power_of_two().max(2);
        let mut total = 0usize;
        while m >= 32 {
            let half = m / 2;
            total = total.saturating_add(2 * half + 2 * half + (half + 1) + (half + 1) + (2 * half + 2));
            m = half;
        }
        self.check_limbs(total.max(a_limbs + b_limbs))
    }

    /// 估算并检查除法商缓冲。
    pub fn check_div(&self, u_limbs: usize, v_limbs: usize) -> Result<()> {
        let q = if v_limbs == 0 { u_limbs + 1 } else { u_limbs.saturating_sub(v_limbs) + 1 };
        self.check_limbs(q.max(u_limbs) + v_limbs + 2)
    }
}

/// 数值执行上下文：预算 + backend 选择钩子。
#[derive(Debug, Clone, Copy)]
pub struct NumericContext {
    budget: ExecutionBudget,
}

impl NumericContext {
    /// 来自 [`crate::backend::PureRustBackend`] 的纯 Rust 默认上限。
    pub fn pure_rust_default() -> Self {
        Self { budget: ExecutionBudget::from_limits(&NumericBackend::contract(&PureRustBackend::default()).limits) }
    }

    /// 由显式 backend / Session 上限构造。
    pub fn from_limits(limits: &NumericBackendLimits) -> Self {
        Self { budget: ExecutionBudget::from_limits(limits) }
    }

    /// 无限制预算（仅测试与内部 convenience；公共 Session 路径勿用）。
    pub fn unlimited() -> Self {
        Self { budget: ExecutionBudget::unlimited() }
    }

    /// 当前预算。
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
