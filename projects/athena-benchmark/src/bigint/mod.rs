//! 统一 bigint 对照：输入、操作、分层与运行器。
//!
//! `athena-bench` 与 Criterion `compare_bigint` 共用本模块；后者只做计时适配。

mod case;
mod operands;
mod runner;

pub use case::{
    BITS, BenchCase, BenchLayer, BigIntOp, ContextPolicy, Implementation, all_cases, cases_for_op,
};
pub use operands::{OperandStrings, PowExp, operand_strings, pow_exp};
pub use runner::{BigIntPrepared, prepare, prepare_all};
