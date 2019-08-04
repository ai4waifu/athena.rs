//! 方言 lowering → Athena 后端的中性请求合同（Living `26`）。
//!
//! 只建立边界枚举，不在本切片实现全部领域能力或方言抽离。
//! 禁止把方言表面赋值 / 作用域 / 模式名写进这些类型。

mod control_plan;
mod domain_goal;
mod session_command;

use athena_types::{Diagnostic, TermId};

pub use control_plan::ControlPlan;
pub use domain_goal::DomainGoal;
pub use session_command::SessionCommand;

/// 一次后端请求（方言 lowering 的目标合同）。
#[derive(Debug, PartialEq)]
pub enum AthenaRequest {
    /// 纯符号项 / 数学项求值或改写入口。
    Term(TermId),
    /// 会话状态变更（定义、清除等）。
    Command(SessionCommand),
    /// 控制流计划（分支、循环、作用域执行）。
    Control(ControlPlan),
    /// 领域目标（Solve、微积分、线代等）。
    Goal(DomainGoal),
}

impl AthenaRequest {
    /// 请求种类标识（诊断 / 观测用，非序列化合同）。
    pub fn kind_name(&self) -> &'static str {
        match self {
            Self::Term(_) => "Term",
            Self::Command(_) => "Command",
            Self::Control(_) => "Control",
            Self::Goal(_) => "Goal",
        }
    }
}

/// 方言 lowering 的结果：要么进入后端合同，要么显式拒绝。
#[derive(Debug, PartialEq)]
pub enum LoweringOutcome {
    /// 已得到中性后端请求。
    Accepted(AthenaRequest),
    /// 无法 lowering（须暴露诊断，禁止回显输入当成功）。
    Rejected(Diagnostic),
}

impl LoweringOutcome {
    /// 接受一项请求。
    pub fn accepted(request: AthenaRequest) -> Self {
        Self::Accepted(request)
    }

    /// 拒绝并附诊断。
    pub fn rejected(diagnostic: Diagnostic) -> Self {
        Self::Rejected(diagnostic)
    }

    /// 是否已接受。
    pub fn is_accepted(&self) -> bool {
        matches!(self, Self::Accepted(_))
    }
}
