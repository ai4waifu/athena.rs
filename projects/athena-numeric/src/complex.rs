//! 复数（骨架；不依赖 `num-complex`）。

use crate::real::Real;

/// 分支策略（与特殊函数 registry 对齐，骨架枚举）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchPolicy {
    /// 主值。
    #[default]
    Principal,
    /// 仅实数。
    RealOnly,
}

/// 复数。
#[derive(Debug, Clone, PartialEq)]
pub struct Complex {
    /// 实部。
    pub re: Real,
    /// 虚部。
    pub im: Real,
    /// 分支。
    pub branch: BranchPolicy,
}

impl Complex {
    /// 由实部构造（虚部 0）。
    pub fn from_real(re: Real) -> Self {
        Self {
            re,
            im: Real::machine(0.0),
            branch: BranchPolicy::Principal,
        }
    }
}
