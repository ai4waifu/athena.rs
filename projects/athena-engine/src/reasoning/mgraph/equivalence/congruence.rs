//! 同余命题索引（按模数隔离的 stable 指纹 union-find）。

use std::collections::HashMap;

/// 单一模数下的指纹等价类（derived index）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ModulusClasses {
    parent: Vec<(u64, u64)>,
}

impl ModulusClasses {
    fn union(&mut self, left: u64, right: u64) {
        if left == right {
            return;
        }
        let root_left = self.find(left);
        let root_right = self.find(right);
        if root_left != root_right {
            self.parent.push((root_right, root_left));
        }
    }

    fn find(&self, id: u64) -> u64 {
        let mut current = id;
        loop {
            match self.parent.iter().rev().find(|&&(child, _)| child == current) {
                Some(&(_, parent)) => current = parent,
                None => return current,
            }
        }
    }

    fn union_count(&self) -> usize {
        self.parent.len()
    }
}

/// 多项式 / 项 stable 指纹上的同余等价类（按 `modulus_fingerprint` 隔离）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CongruenceIndex {
    by_modulus: HashMap<u64, ModulusClasses>,
}

impl CongruenceIndex {
    /// 在给定模数指纹下合并两操作数指纹（单调：只 union）。
    pub fn union(&mut self, modulus_fingerprint: u64, left: u64, right: u64) {
        self.by_modulus
            .entry(modulus_fingerprint)
            .or_default()
            .union(left, right);
    }

    /// 在给定模数指纹下查代表元。
    pub fn find(&self, modulus_fingerprint: u64, id: u64) -> u64 {
        self.by_modulus
            .get(&modulus_fingerprint)
            .map(|classes| classes.find(id))
            .unwrap_or(id)
    }

    /// 已记录 union 边数（跨所有模数）。
    pub fn union_count(&self) -> usize {
        self.by_modulus.values().map(ModulusClasses::union_count).sum()
    }

    /// 已出现的模数指纹个数。
    pub fn modulus_count(&self) -> usize {
        self.by_modulus.len()
    }
}

/// 构造同余命题（模数 + 左右 stable 指纹）。
pub fn congruence_proposition(modulus_fingerprint: u64, left: u64, right: u64) -> crate::reasoning::mgraph::facts::claim::Proposition {
    crate::reasoning::mgraph::facts::claim::Proposition::Congruence {
        modulus_fingerprint,
        left,
        right,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distinct_moduli_do_not_share_equivalence() {
        let mut index = CongruenceIndex::default();
        index.union(7, 10, 20);
        index.union(11, 10, 30);
        assert_eq!(index.find(7, 10), index.find(7, 20));
        assert_ne!(index.find(7, 10), index.find(7, 30));
        assert_eq!(index.find(11, 10), index.find(11, 30));
        assert_eq!(index.modulus_count(), 2);
    }
}
