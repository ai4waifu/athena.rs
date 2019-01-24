//! Magnitude 层：纯 `union Magnitude` + 外层 `meta`（crate-private）。
//!
//! 纪律：
//! - `Magnitude` 不携带 tag；`meta` 是唯一 active-field 来源。
//! - 值对象入口至多一次 mode 分派，之后只向 kernel 传 `LimbView`。
//! - 更新顺序：先写新 storage → 再写新 meta → 最后释放旧 heap。

mod meta;
mod owned;
mod tagged;
mod union;
mod view;

pub(crate) use meta::{Mode, is_negative as meta_is_negative};
pub(crate) use tagged::TaggedMagnitude;
use union::{HeapPayload, Magnitude};

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Magnitude>() == 16);
    assert!(core::mem::size_of::<HeapPayload>() == 16);
    assert!(core::mem::size_of::<TaggedMagnitude>() == 24);
    assert!(core::mem::align_of::<TaggedMagnitude>() == 8);
};
