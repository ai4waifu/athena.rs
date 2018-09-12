//! 伽罗瓦域值对象。

use athena_types::{ExtensionId, FieldId, GroupId};

/// 域自同构（骨架句柄）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Automorphism {
    /// 作用的扩张。
    pub extension: ExtensionId,
    /// 内部标签。
    pub label: String,
}

/// 伽罗瓦群结果（完整性后续填充）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GaloisGroup {
    /// 多项式所在基域。
    pub base_field: FieldId,
    /// 作为置换/抽象群的 id（若已具体化）。
    pub group: Option<GroupId>,
    /// 是否完整算出。
    pub complete: bool,
}

/// 伽罗瓦域返回值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GaloisDomainValue {
    /// 布尔性质（可分 / 正规等）。
    Boolean(bool),
    /// 自同构。
    Automorphism(Automorphism),
    /// 伽罗瓦群。
    GaloisGroup(GaloisGroup),
    /// 占位。
    Placeholder,
}
