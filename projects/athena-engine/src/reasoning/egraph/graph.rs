//! 作用域局部 E-Graph 存储（引导实现）。

use std::collections::HashMap;

use athena_ir::{ApplicationHead, Atom, TermNode, TermStore};
use athena_types::TermId;

use super::ids::{EClassId, ENodeId};

/// 结构化 enode 键（算子 + 子类 id）。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq, Hash)]
pub(crate) struct ENodeKey {
    pub head: ApplicationHead,
    pub children: Vec<EClassId>,
}

impl ENodeKey {
    /// Owning 复制（child class 句柄向量）。
    pub(crate) fn owning_copy(&self) -> Self {
        Self { head: self.head, children: self.children.clone() }
    }
}

/// 单个 enode 载荷。
///
/// **不**实现 [`Clone`]。深复制用 [`Self::owning_copy`]。
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ENode {
    pub key: ENodeKey,
    /// 可选的来源 `TermStore` 根（原子 / 应用根）。
    pub term: Option<TermId>,
    pub eclass: EClassId,
}

impl ENode {
    /// Owning 复制（经 [`ENodeKey::owning_copy`]）。
    pub(crate) fn owning_copy(&self) -> Self {
        Self { key: self.key.owning_copy(), term: self.term, eclass: self.eclass }
    }
}

/// 仅用于 **候选** 搜索的作用域局部等价图。
#[derive(Debug, Default)]
pub struct EGraph {
    /// e-class id 上的并查集父指针（局部，非 ExactUnionFind）。
    parent: Vec<EClassId>,
    /// 按 id 索引的 enode。
    enodes: Vec<ENode>,
    /// Hash-cons：结构键 → enode。
    by_key: HashMap<ENodeKey, ENodeId>,
    /// 项根 → e-class（添加之后）。
    term_class: HashMap<TermId, EClassId>,
}

impl EGraph {
    /// 空图。
    pub fn new() -> Self {
        Self::default()
    }

    /// e-class 数量（含已合并槽位；根数量用 [`Self::eclass_count`]）。
    pub fn len(&self) -> usize {
        self.parent.len()
    }

    /// 是否尚无任何类。
    pub fn is_empty(&self) -> bool {
        self.parent.is_empty()
    }

    /// enode 数量。
    pub fn enode_count(&self) -> usize {
        self.enodes.len()
    }

    /// 不同 e-class 根的数量。
    pub fn eclass_count(&self) -> usize {
        (0..self.parent.len()).map(|i| self.find(EClassId(i as u32)).0).collect::<std::collections::BTreeSet<_>>().len()
    }

    /// 查找代表 e-class。
    pub fn find(&self, id: EClassId) -> EClassId {
        let mut current = id;
        loop {
            let parent = self.parent.get(current.0 as usize).copied().unwrap_or(current);
            if parent == current {
                return current;
            }
            current = parent;
        }
    }

    /// 两个类的候选合并。**不会**触碰 M-Graph。
    pub fn union_classes(&mut self, left: EClassId, right: EClassId) -> bool {
        let root_left = self.find(left);
        let root_right = self.find(right);
        if root_left == root_right {
            return false;
        }
        // 优先保留较小 id 作为根（确定性）。
        let (keep, drop) = if root_left.0 <= root_right.0 { (root_left, root_right) } else { (root_right, root_left) };
        if let Some(slot) = self.parent.get_mut(drop.0 as usize) {
            *slot = keep;
        }
        true
    }

    /// 此前加入本图的项（会话局部根）。
    pub fn known_terms(&self) -> Vec<TermId> {
        self.term_class.keys().copied().collect()
    }

    /// `class` 中记录的全部项根（经 find 之后）。
    pub fn terms_in_class(&self, class: EClassId) -> Vec<TermId> {
        let root = self.find(class);
        let mut out = Vec::new();
        for (term, c) in &self.term_class {
            if self.find(*c) == root {
                out.push(*term);
            }
        }
        out.sort_by_key(|t| t.0);
        out.dedup();
        out
    }

    /// 查找此前添加项的 e-class。
    pub fn class_of_term(&self, term: TermId) -> Option<EClassId> {
        self.term_class.get(&term).map(|c| self.find(*c))
    }

    /// 某 e-class 首次记录的项根（引导抽取）。
    pub fn term_for_class(&self, class: EClassId) -> Option<TermId> {
        let root = self.find(class);
        for (term, c) in &self.term_class {
            if self.find(*c) == root {
                return Some(*term);
            }
        }
        for node in &self.enodes {
            if self.find(node.eclass) == root {
                if let Some(term) = node.term {
                    return Some(term);
                }
            }
        }
        None
    }

    /// 将 `TermStore` 根加入图（结构化递归）。
    pub fn add_term(&mut self, store: &TermStore, term: TermId) -> Option<EClassId> {
        if let Some(existing) = self.term_class.get(&term).copied() {
            return Some(self.find(existing));
        }
        let class = match store.get(term)? {
            TermNode::Atom(atom) => self.add_atom(term, atom),
            TermNode::Collection { elements, .. } => {
                // 集合：引导阶段当作不透明的类原子叶子（无 List 语义）。
                let _ = elements;
                self.alloc_class_for_term(term)
            }
            TermNode::Application { head, arguments } => {
                let mut children = Vec::with_capacity(arguments.len());
                for arg in arguments {
                    children.push(self.add_term(store, *arg)?);
                }
                self.add_application(term, *head, children)
            }
        };
        self.term_class.insert(term, class);
        Some(class)
    }

    fn add_atom(&mut self, term: TermId, atom: &Atom) -> EClassId {
        let _ = atom;
        self.alloc_class_for_term(term)
    }

    fn add_application(&mut self, term: TermId, head: ApplicationHead, children: Vec<EClassId>) -> EClassId {
        let key = ENodeKey { head, children };
        if let Some(id) = self.by_key.get(&key).copied() {
            let class = self.enodes[id.0 as usize].eclass;
            return self.find(class);
        }
        let class = self.alloc_class();
        let enode_id = ENodeId(self.enodes.len() as u32);
        self.enodes.push(ENode { key: key.owning_copy(), term: Some(term), eclass: class });
        self.by_key.insert(key, enode_id);
        class
    }

    fn alloc_class_for_term(&mut self, term: TermId) -> EClassId {
        let class = self.alloc_class();
        // 避免为叶子分配空子节点 + 哨兵扩展头：
        // 叶子在抽取器需要结构之前仅作为 class。
        let _ = term;
        class
    }

    fn alloc_class(&mut self) -> EClassId {
        let id = EClassId(self.parent.len() as u32);
        self.parent.push(id);
        id
    }
}
