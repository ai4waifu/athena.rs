//! 闭包步进（骨架）。

pub mod operational;

use athena_types::{Diagnostic, DiagnosticCode};

use crate::reasoning::mgraph::core::state::MGraphState;

pub use operational::OperationalState;

/// 闭包资源限制。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClosureLimits {
    /// 最大步数。
    pub max_steps: u32,
}

impl Default for ClosureLimits {
    fn default() -> Self {
        Self { max_steps: 1024 }
    }
}

/// 闭包结果（骨架；完整版含 RuntimeValue）。
#[derive(Debug, Clone, PartialEq)]
pub struct ClosureResult {
    /// 终态。
    pub state: MGraphState,
    /// 是否在限额内完成饱和。
    pub complete: bool,
    /// 诊断。
    pub diagnostics: Vec<Diagnostic>,
}

/// 运行单步闭包（骨架：不修改状态，标记 incomplete）。
pub fn run_closure_step(state: &MGraphState, _limits: &ClosureLimits) -> ClosureResult {
    ClosureResult {
        state: state.clone(),
        complete: false,
        diagnostics: vec![
            Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "mgraph")
                .detail("operation", "closure_step"),
        ],
    }
}
