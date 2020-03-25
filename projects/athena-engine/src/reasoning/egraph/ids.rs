//! 单个作用域局部 [`super::EGraph`] 内的稳定句柄。

/// 等价类标识（仅对单个 E-Graph 实例局部有效）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EClassId(pub u32);

/// 单个 E-Graph 内的 enode 标识（算子 + 子类）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ENodeId(pub u32);
