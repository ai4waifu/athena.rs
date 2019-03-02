//! 算法策略层：选数学路径，不写 ISA、不分配、不读 `Magnitude` union。

mod div;
mod gcd;
mod mul;
mod planner;

pub use div::{DIV_BZ_THRESHOLD, DivStrategy};
pub(crate) use div::select_div_strategy;
pub use gcd::{GCD_HALF_THRESHOLD, GCD_LEHMER_THRESHOLD, GcdStrategy};
pub(crate) use gcd::select_gcd_strategy;
pub use mul::{MUL_KARATSUBA_THRESHOLD, MUL_TOOM_THRESHOLD, MulStrategy, karatsuba_scratch_limbs, toom3_scratch_limbs};
pub(crate) use mul::select_mul_strategy;
pub use planner::AlgorithmPlanner;
