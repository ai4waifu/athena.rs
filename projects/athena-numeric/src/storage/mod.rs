//! Storage：纯 `union Magnitude`；外层值自有 `meta`（crate-private 物理辅助）。
//!
//! 纪律：
//! - `Magnitude` 不携带 tag；外层 `meta` 是唯一 active-field 来源（解释因类型而异）。
//! - 无跨类型 `TaggedMagnitude` 语义中间层。
//! - 值对象入口至多一次 mode 分派，之后只向 kernel 传 `LimbView`。
//! - 更新顺序：先写新 storage → 再写新 meta → 最后释放旧 heap。
//!
//! 本模块即四层正交中的 storage 层（原目录名 `magnitude/`）。

mod gc_err;
mod meta;
mod owned;
mod pair;
mod union;
mod view;

pub(crate) use gc_err::gc_alloc_error;
pub(crate) use meta::Mode;
pub(crate) use pair::MagnitudePair;
use union::{HeapPayload, Magnitude};

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Magnitude>() == 16);
    assert!(core::mem::size_of::<HeapPayload>() == 16);
    assert!(core::mem::size_of::<MagnitudePair>() == 24);
    assert!(core::mem::align_of::<MagnitudePair>() == 8);
};
