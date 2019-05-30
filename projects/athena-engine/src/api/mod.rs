//! 对外引擎 API（句柄、选项与中性请求合同）。
//!
//! 实现放在私有 `engine` 子模块，避免 `api::engine::…` 与 `api::…` 双路径。

mod engine;
pub mod request;

pub use engine::{AthenaEngine, EvalOptions, SimplifyOptions};
pub use request::{
    AthenaRequest, ControlPlan, DefinitionEvaluationTiming, DomainGoal, LoweringOutcome, SessionCommand,
};
