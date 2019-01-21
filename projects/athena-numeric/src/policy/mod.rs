//! 运行策略（预算 / 取消 / backend 上限），不参与值身份。

pub mod backend_limits;
pub mod execution_budget;

pub use backend_limits::NumericBackendLimits;
pub use execution_budget::{ExecutionBudget, NumericContext};
