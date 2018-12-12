//! 算法需求与存储 capability 合同。

/// 算法对工作集与 storage 的要求。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphAlgorithmRequirements {
    /// 需要邻接按目标节点排序（CSR/CSC 天然满足）。
    pub sorted_adjacency: bool,
    /// 需要反向邻接（反向图或 CSC）。
    pub reverse_adjacency: bool,
    /// 需要随机访问 offsets/indices。
    pub random_access: bool,
    /// 至少需要的完整扫描遍数。
    pub min_passes: u32,
    /// 允许 frontier/visited 落外存工作区。
    pub external_workspace: bool,
    /// 允许分块顺序扫描邻接。
    pub chunked_sequential: bool,
}

impl GraphAlgorithmRequirements {
    /// 小图内存 BFS/DFS/components。
    pub const fn in_memory_traversal() -> Self {
        Self {
            sorted_adjacency: false,
            reverse_adjacency: false,
            random_access: false,
            min_passes: 1,
            external_workspace: false,
            chunked_sequential: false,
        }
    }

    /// storage-backed CSR 顺序扫描。
    pub const fn chunked_csr_scan() -> Self {
        Self {
            sorted_adjacency: true,
            reverse_adjacency: false,
            random_access: true,
            min_passes: 1,
            external_workspace: true,
            chunked_sequential: true,
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
}

impl GraphCapabilities {
    /// 是否满足 [`GraphAlgorithmRequirements`]。
    pub fn satisfies(self, req: GraphAlgorithmRequirements) -> bool {
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
        if req.min_passes > 1 && !self.in_memory && !self.chunked_sequential {
            return false;
        }
        true
    }
}
