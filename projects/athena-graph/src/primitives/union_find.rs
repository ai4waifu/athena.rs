//! 并查集（结构原语，非图论 claim）。

/// 并查集（结构原语）。
#[derive(Debug, Clone)]
pub struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl UnionFind {
    /// 创建 `size` 个独立集合 `{0..size}`。
    pub fn new(size: usize) -> Self {
        Self { parent: (0..size).collect(), rank: vec![0; size] }
    }

    /// 查找代表元（路径压缩）。
    pub fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    /// 合并集合；返回是否实际合并。
    pub fn union(&mut self, a: usize, b: usize) -> bool {
        let mut ra = self.find(a);
        let mut rb = self.find(b);
        if ra == rb {
            return false;
        }
        if self.rank[ra] < self.rank[rb] {
            std::mem::swap(&mut ra, &mut rb);
        }
        self.parent[rb] = ra;
        if self.rank[ra] == self.rank[rb] {
            self.rank[ra] = self.rank[ra].saturating_add(1);
        }
        true
    }

    /// 当前集合个数。
    pub fn set_count(&mut self) -> usize {
        let n = self.parent.len();
        let mut roots = vec![false; n];
        for i in 0..n {
            roots[self.find(i)] = true;
        }
        roots.iter().filter(|&&b| b).count()
    }
}
