//! 算法需求与存储 capability 合同。

/// 算法对工作集与 storage 的要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphAlgorithmRequirements {
    /// 要求完整图驻留内存（对应 Living `InMemoryOnly`）。
    pub require_in_memory: bool,
    /// 需要邻接按目标节点排序（CSR/CSC 天然满足）。
    pub sorted_adjacency: bool,
    /// 需要反向邻接（反向图或 CSC）。
    pub reverse_adjacency: bool,
    /// 需要随机访问 offsets/indices（对应 `RandomAccessStorage`）。
    pub random_access: bool,
    /// 至少需要的完整扫描遍数（`>1` 时对应 `MultiPass`）。
    pub min_passes: u32,
    /// 允许 frontier/visited 落外存工作区（对应 `ExternalWorkspace`）。
    pub external_workspace: bool,
    /// 允许分块顺序扫描邻接（对应 `ChunkedSequential`）。
    pub chunked_sequential: bool,
    /// 需要分片分布式存储（对应 `DistributedShards`）。
    pub distributed_shards: bool,
}

impl GraphAlgorithmRequirements {
    /// 小图内存 BFS/DFS/components（`InMemoryOnly` 软需求：不强制 `require_in_memory`，由调用方选择）。
    pub const fn in_memory_traversal() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 1,
            external_workspace: false,
            chunked_sequential: false,
            distributed_shards: false,
        }
    }

    /// 必须整图驻留内存。
    pub const fn in_memory_only() -> Self {
        Self {
            require_in_memory: true,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 1,
            external_workspace: false,
            chunked_sequential: false,
            distributed_shards: false,
        }
    }

    /// storage-backed CSR 顺序扫描。
    pub const fn chunked_csr_scan() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: true,
            reverse_adjacency: false,
            random_access: true,
            min_passes: 1,
            external_workspace: true,
            chunked_sequential: true,
            distributed_shards: false,
        }
    }

    /// 仅要求分块顺序扫描（`ChunkedSequential`）。
    pub const fn chunked_sequential() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 1,
            external_workspace: false,
            chunked_sequential: true,
            distributed_shards: false,
        }
    }

    /// 外存工作区（`ExternalWorkspace`）。
    pub const fn external_workspace() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 1,
            external_workspace: true,
            chunked_sequential: false,
            distributed_shards: false,
        }
    }

    /// 随机访问存储（`RandomAccessStorage`）。
    pub const fn random_access_storage() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: true,
            min_passes: 1,
            external_workspace: false,
            chunked_sequential: false,
            distributed_shards: false,
        }
    }

    /// 多遍扫描（`MultiPass`，`min_passes = 2`）。
    pub const fn multi_pass() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 2,
            external_workspace: false,
            chunked_sequential: false,
            distributed_shards: false,
        }
    }

    /// 分片分布式存储（`DistributedShards`）。
    pub const fn distributed_shards() -> Self {
        Self {
            require_in_memory: false,
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 1,
            external_workspace: false,
            chunked_sequential: false,
            distributed_shards: true,
        }
    }
}

/// 图表示当前提供的能力。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphCapabilities {
    /// 完整图驻留进程内存。
    pub in_memory: bool,
    /// 出邻接（或等价）按目标排序。
    pub sorted_adjacency: bool,
    /// 可枚举入邻接（反向索引或 CSC）。
    pub reverse_adjacency: bool,
    /// offsets/indices 随机范围读。
    pub random_access: bool,
    /// 邻接可分块顺序读。
    pub chunked_sequential: bool,
    /// 工作区可落外存 storage。
    pub external_workspace: bool,
    /// 图数据已按分片分布（当前 bootstrap 恒为 false）。
    pub distributed_shards: bool,
}

impl GraphCapabilities {
    /// 是否满足 [`GraphAlgorithmRequirements`]。
    ///
    /// **不满足时调用方必须结构化失败**，禁止偷偷整图物化以「凑」capability。
    pub fn satisfies(self, req: GraphAlgorithmRequirements) -> bool {
        if req.require_in_memory && !self.in_memory {
            return false;
        }
        if req.sorted_adjacency && !self.sorted_adjacency {
            return false;
        }
        if req.reverse_adjacency && !self.reverse_adjacency {
            return false;
        }
        if req.random_access && !self.random_access {
            return false;
        }
        if req.external_workspace && !self.external_workspace {
            return false;
        }
        if req.chunked_sequential && !self.chunked_sequential {
            return false;
        }
        if req.distributed_shards && !self.distributed_shards {
            return false;
        }
        if req.min_passes > 1 && !self.in_memory && !self.chunked_sequential {
            return false;
        }
        true
    }
}
