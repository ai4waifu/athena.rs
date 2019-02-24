//! 统一 bigint 对照：输入、操作、分层与运行器。
//!
//! - `athena-bench`：校验矩阵 + 资源采样（不计时）
//! - Criterion `compare_bigint`：唯一 ns/op 计时入口

mod case;
mod operands;
mod runner;

pub use case::{
    BITS, BenchCase, BenchLayer, BigIntOp, ContextPolicy, Implementation, all_cases, cases_for_op,
};
pub use operands::{OperandStrings, PowExp, operand_strings, pow_exp};
pub use runner::{BigIntPrepared, prepare, prepare_all};
