//! 算法策略层：选数学路径，不写 ISA、不分配、不读 `Magnitude` union。

mod div;
mod mul;
mod planner;

pub use div::{DIV_BZ_THRESHOLD, DivStrategy, select_div_strategy};
pub use mul::{
    MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, MulStrategy, karatsuba_scratch_limbs, select_mul_strategy, toom3_scratch_limbs,
};
pub use planner::AlgorithmPlanner;
