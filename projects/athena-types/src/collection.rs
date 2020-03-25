//! 中性集合种类（· 禁止万能 `List` 语义）。

/// 领域专用集合种类句柄（Session / registry 本地）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CollectionKindId(pub u32);

/// 显式集合 / 序列 / 矩阵结构种类。
///
/// 方言 lowering 必须选出具体种类。禁止把参数序列、有序集合、矩阵行与模式序列混成同一默认容器。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CollectionKind {
    /// 结构序列（参数列表、模式序列等，无集合代数语义）。
    StructuralSequence,
    /// 元组。
    Tuple,
    /// 有序集合（例如前端 `List` lowering 的目标之一，不是引擎默认万能容器）。
    OrderedCollection,
    /// 无序 / 集合式集合（语义由领域与算子决定）。
    SetLikeCollection,
    /// 向量。
    Vector,
    /// 矩阵行。
    MatrixRow,
    /// 矩阵列。
    MatrixColumn,
    /// 矩阵。
    Matrix,
    /// 领域注册的集合种类。
    DomainCollection(CollectionKindId),
}

impl CollectionKind {
    /// 诊断 / 调试标签（非方言表面名 · ）。
    pub const fn debug_label(self) -> &'static str {
        match self {
            Self::StructuralSequence => "StructuralSequence",
            Self::Tuple => "Tuple",
            Self::OrderedCollection => "OrderedCollection",
            Self::SetLikeCollection => "SetLikeCollection",
            Self::Vector => "Vector",
            Self::MatrixRow => "MatrixRow",
            Self::MatrixColumn => "MatrixColumn",
            Self::Matrix => "Matrix",
            Self::DomainCollection(_) => "DomainCollection",
        }
    }
}
