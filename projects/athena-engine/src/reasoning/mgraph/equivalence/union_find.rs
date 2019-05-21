//! 无条件 exact equality union-find（derived index；可自 fact log 重建）。

use athena_types::ExprId;

/// 仅缓存 `Unconditional + ProvenExact` 等式投影。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExactUnionFind {
    parent: Vec<(ExprId, ExprId)>,
}

impl ExactUnionFind {
    /// 合并两项（单调：只 union，不 split）。
    pub fn union(&mut self, left: ExprId, right: ExprId) {
        if left == right {
            return;
        }
        let root_left = self.find(left);
        let root_right = self.find(right);
        if root_left != root_right {
            self.parent.push((root_right, root_left));
        }
    }

    /// 查代表元。
    pub fn find(&self, id: ExprId) -> ExprId {
        let mut current = id;
        loop {
            match self.parent.iter().rev().find(|&&(child, _)| child == current) {
                Some(&(_, parent)) => current = parent,
                None => return current,
            }
        }
    }

    /// 已记录 union 边数。
    pub fn union_count(&self) -> usize {
        self.parent.len()
    }
}
