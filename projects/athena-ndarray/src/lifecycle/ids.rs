//! Array*Id 与 wire revision / snapshot。

use std::sync::atomic::{AtomicU64, Ordering};

use athena_gc::GcObjectId;

use crate::{ArrayLayout, LogicalShape};

static NEXT_ARRAY_ID: AtomicU64 = AtomicU64::new(1);

/// Session/local 逻辑数组身份（≠ shape）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ArrayId(pub u64);

impl ArrayId {
    /// 分配新身份。
    pub fn allocate() -> Self {
        Self(NEXT_ARRAY_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// 由原始值构造。
    pub const fn from_raw(id: u64) -> Self {
        Self(id)
    }
}

/// 单调修订号（wire）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Default)]
pub struct ArrayRevision(pub u64);

impl ArrayRevision {
    /// 饱和递增。
    pub fn bump(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

/// Wire 快照（不含 GC 对象表）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArraySnapshot {
    /// 逻辑数组。
    pub array_id: ArrayId,
    /// 修订号码。
    pub revision: ArrayRevision,
    /// 逻辑 shape。
    pub shape: LogicalShape,
    /// 布局（表示，≠ 身份）。
    pub layout: ArrayLayout,
}

impl ArraySnapshot {
    /// 构造。
    pub fn new(array_id: ArrayId, revision: ArrayRevision, shape: LogicalShape, layout: ArrayLayout) -> Self {
        Self { array_id, revision, shape, layout }
    }
}

/// 不可变版本记录的 GC 身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ArrayRevisionId {
    /// 底层 GC 对象身份。
    pub object: GcObjectId,
}

impl ArrayRevisionId {
    /// 由 [`GcObjectId`] 构造。
    pub const fn from_object(object: GcObjectId) -> Self {
        Self { object }
    }

    /// 底层对象身份。
    pub const fn as_object(self) -> GcObjectId {
        self.object
    }
}

/// 算法可读稳定观测身份。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ArraySnapshotId {
    /// 底层 GC 对象身份。
    pub object: GcObjectId,
}

impl ArraySnapshotId {
    /// 由 [`GcObjectId`] 构造。
    pub const fn from_object(object: GcObjectId) -> Self {
        Self { object }
    }

    /// 底层对象身份。
    pub const fn as_object(self) -> GcObjectId {
        self.object
    }
}

/// 物理元素块身份（≠ [`ArrayId`]）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct ArrayChunkId {
    /// 底层 GC 对象身份。
    pub object: GcObjectId,
}

impl ArrayChunkId {
    /// 由 [`GcObjectId`] 构造。
    pub const fn from_object(object: GcObjectId) -> Self {
        Self { object }
    }

    /// 底层对象身份。
    pub const fn as_object(self) -> GcObjectId {
        self.object
    }
}
