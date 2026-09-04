//! 领域目标入口（中性边界 · 复用现有 `DomainRequest` 载荷）。

use crate::domains::dispatch::DomainRequest;

/// 领域目标。Solve / 微积分 / 线代等走此路径。
///
/// 本切片只建立边界：直接包装现有 [`DomainRequest`]。
/// 后续可按域拆细中性 goal 类型，但禁止再引入方言表面名。
#[derive(Debug, PartialEq)]
pub enum DomainGoal {
    /// 经统一域分派入口执行。
    Dispatch(DomainRequest),
}
