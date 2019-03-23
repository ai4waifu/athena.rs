//! 可达性 ≠ 驻留：chunk 存储状态机。

/// Chunk 驻留 / 映射状态（与语义可达性正交）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ChunkResidency {
    /// 内存中。
    #[default]
    Resident,
    /// 文件映射中。
    Mapped,
    /// 已 spill，仅有 backing。
    Spilled,
    /// 正在加载。
    Loading,
    /// 可被 LRU 淘汰 resident copy（仍可达时可保留 backing）。
    Evictable,
    /// 加载或映射失败。
    Failed,
}

impl ChunkResidency {
    /// 当前是否持有可解析的 resident / mapped 地址语义。
    pub const fn has_address(self) -> bool {
        matches!(self, Self::Resident | Self::Mapped)
    }

    /// 是否允许发起 materialize（从 spill / failed 恢复）。
    pub const fn can_materialize(self) -> bool {
        matches!(self, Self::Spilled | Self::Failed | Self::Evictable)
    }
}
