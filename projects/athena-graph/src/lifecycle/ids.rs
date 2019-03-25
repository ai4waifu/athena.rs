//! Graph*Id / SpillObjectId（`GcObjectId` 族 newtype）。

use std::sync::atomic::{AtomicU32, Ordering};

use athena_gc::GcObjectId;

static NEXT_LIFECYCLE_INDEX: AtomicU32 = AtomicU32::new(1);

/// 分配引导用 lifecycle 对象身份（带 generation，可后续绑定真实 heap slot）。
pub fn allocate_lifecycle_object_id() -> GcObjectId {
    GcObjectId { index: NEXT_LIFECYCLE_INDEX.fetch_add(1, Ordering::Relaxed), generation: 1 }
}

macro_rules! lifecycle_id {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
        pub struct $name {
            /// 底层 GC 对象身份。
            pub object: GcObjectId,
        }

        impl $name {
            /// 分配新身份。
            pub fn allocate() -> Self {
                Self {
                    object: allocate_lifecycle_object_id(),
                }
            }

            /// 由已有 [`GcObjectId`] 构造（须来自同一生命周期表）。
            pub const fn from_object(object: GcObjectId) -> Self {
                Self { object }
            }

            /// 底层对象身份。
            pub const fn as_object(self) -> GcObjectId {
                self.object
            }
        }
    };
}

lifecycle_id!(
    /// 一次不可变结构版本记录的可追踪身份（≠ [`crate::GraphRevision`] 号码）。
    GraphRevisionId
);
lifecycle_id!(
    /// 算法可读稳定观测身份。
    GraphSnapshotId
);
lifecycle_id!(
    /// 物理存储块身份（≠ [`crate::GraphId`]）。
    GraphChunkId
);
lifecycle_id!(
    /// 派生视图身份。
    GraphViewId
);
lifecycle_id!(
    /// 算法工作区 / checkpoint 身份。
    GraphWorkspaceId
);
lifecycle_id!(
    /// out-of-core backing 身份（≠ resident 地址）。
    SpillObjectId
);
