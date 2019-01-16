//! 后端无关的 limb / 浮点内核合同与缓冲。

pub(crate) mod buffer;
pub(crate) mod limb;

pub(crate) use buffer::{LimbBuffer, ScratchWorkspace, kernel_err};
