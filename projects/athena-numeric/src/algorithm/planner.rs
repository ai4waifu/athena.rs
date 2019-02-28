//! 操作规划：按宽度 / 能力选算法（非热循环内 CPU detect）。

use crate::dispatch::CapabilityBundle;

use super::{DivStrategy, GcdStrategy, MulStrategy, select_div_strategy, select_gcd_strategy, select_mul_strategy};

/// 算法规划器（持有冻结的 capability 视图）。
#[derive(Debug, Clone, Copy)]
pub struct AlgorithmPlanner {
    caps: CapabilityBundle,
}

impl AlgorithmPlanner {
    /// 由 context 级能力束构造。
    pub fn new(caps: CapabilityBundle) -> Self {
        Self { caps }
    }

    /// 当前能力束。
    pub fn capabilities(&self) -> CapabilityBundle {
        self.caps
    }

    /// 规划乘法。
    pub fn plan_mul(&self, a_limbs: usize, b_limbs: usize) -> MulStrategy {
        select_mul_strategy(a_limbs, b_limbs, self.caps.algorithm)
    }

    /// 规划除法。
    pub fn plan_div(&self, u_limbs: usize, v_limbs: usize) -> DivStrategy {
        select_div_strategy(u_limbs, v_limbs, self.caps.algorithm)
    }

    /// 规划 GCD。
    pub fn plan_gcd(&self, a_limbs: usize, b_limbs: usize) -> GcdStrategy {
        select_gcd_strategy(a_limbs, b_limbs, self.caps.algorithm)
    }
}
