//! GC / arena 身份类型。

/// Segment 稳定索引（复用时 generation 递增）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentId {
    /// 表内下标。
    pub index: u32,
    /// 代际（防 ABA）。
    pub generation: u32,
}

/// 图 / 值对象句柄（图层引用；kernel 不用此查 limb）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GcObjectId {
    /// 对象表下标。
    pub index: u32,
    /// 代际。
    pub generation: u32,
}

/// Root 登记令牌（注销时使用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootToken(pub u64);
