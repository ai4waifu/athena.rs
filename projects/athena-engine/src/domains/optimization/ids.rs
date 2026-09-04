//! Session-local 查找句柄（跨 Session 身份见 [`super::OptimizationFingerprint`]）。

/// 优化问题句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProblemId(pub u32);

/// 决策变量句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct VariableId(pub u32);

/// 约束句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConstraintId(pub u32);

/// 目标句柄。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectiveId(pub u32);
