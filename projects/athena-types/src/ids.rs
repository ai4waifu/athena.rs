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

/// 群对象 id（Session / 注册表句柄）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupId(pub u32);

/// 群元素 id（绑定所属群，禁止跨群运算）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GroupElementId(pub u32);

/// 域对象 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct FieldId(pub u32);

/// 域扩张 id。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ExtensionId(pub u32);

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
