//! 目标函数。

use athena_types::ExprId;

use super::ids::ObjectiveId;

/// 优化方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectiveSense {
    /// 最小化。
    Minimize,
    /// 最大化。
    Maximize,
}

/// 目标。
#[derive(Debug, Clone, PartialEq)]
pub struct Objective {
    /// Session-local id。
    pub id: ObjectiveId,
    /// 方向。
    pub sense: ObjectiveSense,
    /// 目标表达式。
    pub expression: ExprId,
    /// 多目标优先级（越小越优先；单目标为 0）。
    pub priority: u32,
}
