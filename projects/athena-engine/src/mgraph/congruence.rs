//! 同余命题索引（stable 指纹 union-find；F5 合同）。

use std::collections::HashMap;

/// 多项式 / 项 stable 指纹上的同余等价类（derived index）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CongruenceIndex {
    parent: Vec<(u64, u64)>,
    roots: HashMap<u64, u64>,
}

impl CongruenceIndex {
    /// 合并两指纹（单调：只 union）。
    pub fn union(&mut self, left: u64, right: u64) {
        if left == right {
            return;
        }
        let root_left = self.find(left);
        let root_right = self.find(right);
        if root_left != root_right {
            self.parent.push((root_right, root_left));
            self.roots.insert(root_left, root_left);
            self.roots.insert(root_right, root_left);
        }
    }

    /// 查代表元。
    pub fn find(&self, id: u64) -> u64 {
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

/// 构造同余命题（模数 + 左右 stable 指纹）。
pub fn congruence_proposition(modulus_fingerprint: u64, left: u64, right: u64) -> super::claim::Proposition {
    super::claim::Proposition::Congruence { modulus_fingerprint, left, right }
}
