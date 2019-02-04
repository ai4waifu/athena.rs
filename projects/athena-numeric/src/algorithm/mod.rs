//! 算法策略层：选数学路径，不写 ISA、不分配、不读 `Magnitude` union。

mod mul;
mod planner;

pub use mul::{MUL_KARATSUBA_THRESHOLD, MulStrategy, karatsuba_scratch_limbs, select_mul_strategy};
pub use planner::AlgorithmPlanner;
