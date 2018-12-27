//! 稳定标识符 newtype（IR 与注册表）。

/// Core term id（arena 索引）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TermId(pub u32);

/// IR 节点 id（预留，与 term 区分 stmt 等扩展）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId(pub u32);

/// 符号 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SymbolId(pub u32);

/// 内建 / 注册算子 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OperatorId(pub u32);

/// 数学域 id（系数域等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DomainId(pub u32);

/// 群对象 id（**Session-local** 查找句柄；跨 Session 用 fingerprint）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub u32);

/// 群元素 id（绑定所属群，禁止跨群运算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupElementId(pub u32);

/// 域对象 id（**Session-local** 查找句柄；跨 Session 用 fingerprint）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub u32);

/// 域扩张 id（**Session-local**）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtensionId(pub u32);

/// 系数环 id（ℤ / ℚ / 𝔽_p / ℤ/nℤ / 有限域等；Session 内 intern 句柄）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CoefficientRingId(pub u32);

/// 多项式环 id（**Session-local** 查找句柄；跨 Session 语义身份为 `RingFingerprint`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RingId(pub u32);

/// 擦除后的 presentation 句柄（仅跨域共享骨架；新代码用强类型）。
///
/// Session-local：数值相等不代表跨 Session 同一表示。跨 Session / 缓存 / 序列化用 fingerprint。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PresentationId(pub u32);

/// 域 presentation 句柄（Session-local；禁止与 [`GroupPresentationId`] 混用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldPresentationId(pub u32);

/// 群 presentation 句柄（Session-local；禁止与 [`FieldPresentationId`] 混用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupPresentationId(pub u32);

/// 代数映射 id（嵌入、同态、商投影等）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AlgebraMapId(pub u32);

/// 域自同构 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AutomorphismId(pub u32);

/// 子群 id（含 inclusion 映射引用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubgroupId(pub u32);

/// 假设集合 id（Session / 请求附着）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AssumptionSetId(pub u32);

/// 源码位置（字节偏移）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SourceSpan {
    /// 起始字节（含）。
    pub start: u32,
    /// 结束字节（不含）。
    pub end: u32,
}

/// IR / wire 序列化 schema 版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SerializationVersion(pub u16);

impl SerializationVersion {
    /// 当前 schema。
    pub const CURRENT: Self = Self(1);
}
