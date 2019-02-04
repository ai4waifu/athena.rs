//! 可选 native limb 加速（feature `native-accelerated`）。
//!
//! 合同：Athena 提供 `&[u64]` / `&mut [u64]`；本适配器绑定 ISA/`KernelTable`，
//! 不持有 foreign bigint 对象。启用 feature 后默认走主机 ISA 表（与 pure Rust parity）。

use crate::{
    dispatch::{CapabilityBundle, MachineCapability},
    kernel::KernelTable,
};

/// Native limb 加速适配器：暴露已绑定的 [`KernelTable`]。
#[derive(Debug, Clone, Copy, Default)]
pub struct NativeAcceleratedAdapter;

impl NativeAcceleratedAdapter {
    /// 按主机 `MachineCapability` 绑定 kernel 表。
    pub fn kernel_table() -> KernelTable {
        KernelTable::bind(MachineCapability::detect_host())
    }

    /// 推荐的能力束（主机 ISA + 默认算法/资源）。
    pub fn capability_bundle() -> CapabilityBundle {
        CapabilityBundle::host_default()
    }
}
