//! `TermStore` — Core IR 唯一符号项存储（结构 hash-cons）。

use std::collections::HashMap;

use athena_types::{CollectionKind, Diagnostic, DiagnosticCode, Result, SourceSpan, TermId, TermRef};

use crate::{
    canonical::fnv1a64,
    node::{Atom, TermNode},
    operator::ApplicationHead,
    symbol::SymbolTable,
};

const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// Core CAS IR 符号项存储。
#[derive(Debug)]
pub struct TermStore {
    nodes: Vec<TermNode>,
    spans: Vec<SourceSpan>,
    symbols: SymbolTable,
    /// 结构 hash → 候选 id（经 [`PartialEq`] 校验冲突）。
    by_hash: HashMap<u64, Vec<TermId>>,
    /// 整库代际（过渡：reset / 未来 reclaim 时递增；[`TermRef`] 校验用）。
    epoch: u32,
}

impl Default for TermStore {
    fn default() -> Self {
        Self { nodes: Vec::new(), spans: Vec::new(), symbols: SymbolTable::default(), by_hash: HashMap::new(), epoch: 1 }
    }
}

impl TermStore {
    /// 空存储。
    pub fn new() -> Self {
        Self::default()
    }

    /// 当前 store epoch（写入 [`TermRef::generation`]）。
    #[inline]
    pub fn epoch(&self) -> u32 {
        self.epoch
    }

    /// 将裸 [`TermId`] 提升为带当前 epoch 的 [`TermRef`]。
    ///
    /// 若 id 越界返回 `None`（不推进 epoch）。
    pub fn term_ref(&self, id: TermId) -> Option<TermRef> {
        if (id.0 as usize) < self.nodes.len() { Some(TermRef::new(id, self.epoch)) } else { None }
    }

    /// 校验 [`TermRef`] 仍指向本 store 当前代际中的有效节点。
    pub fn check_ref(&self, term: TermRef) -> Result<TermId> {
        if term.generation != self.epoch {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "TermStore")
                .detail("reason", "stale_term_generation")
                .detail("expected", self.epoch)
                .detail("actual", term.generation)
                .detail("term", term.id.0));
        }
        if (term.id.0 as usize) >= self.nodes.len() {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("component", "TermStore")
                .detail("reason", "term_out_of_range")
                .detail("term", term.id.0));
        }
        Ok(term.id)
    }

    /// 推进 epoch（测试 / 未来 clear·reclaim）。现有裸 [`TermId`] 经 [`TermRef`] 将判 stale。
    pub fn bump_epoch(&mut self) {
        self.epoch = self.epoch.wrapping_add(1).max(1);
    }

    /// 符号 intern 表。
    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /// 可变符号表（builder 使用）。
    pub fn symbols_mut(&mut self) -> &mut SymbolTable {
        &mut self.symbols
    }

    /// 分配或复用 term 节点（结构 hash-cons），返回稳定 [`TermId`]。
    ///
    /// 相同结构与载荷共享同一 id。命中时保留首次插入的
    /// [`SourceSpan`]。
    pub fn push(&mut self, kind: TermNode, span: SourceSpan) -> TermId {
        let hash = structure_hash(&kind);
        if let Some(ids) = self.by_hash.get(&hash) {
            for &id in ids {
                if self.nodes.get(id.0 as usize) == Some(&kind) {
                    return id;
                }
            }
        }
        let id = TermId(self.nodes.len() as u32);
        self.nodes.push(kind);
        self.spans.push(span);
        self.by_hash.entry(hash).or_default().push(id);
        id
    }

    /// 按 id 取 term 节点。
    pub fn get(&self, id: TermId) -> Option<&TermNode> {
        self.nodes.get(id.0 as usize)
    }

    /// 按 id 取 span。
    pub fn span(&self, id: TermId) -> Option<SourceSpan> {
        self.spans.get(id.0 as usize).copied()
    }

    /// 节点数量。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 结构完整性检查（索引有效、无环）。
    pub fn verify(&self, root: TermId) -> Result<()> {
        verify_term(self, root, &mut vec![])
    }

    /// 结构等价（数值载荷按 [`NumericValue`](athena_numeric::NumericValue) 精确相等）。
    ///
    /// DAG 共享子图去重；与插入地址无关，只比结构与载荷。Hash-cons 后同结构常为同一 [`TermId`]。
    pub fn structural_eq(&self, a: TermId, b: TermId) -> bool {
        let mut seen = std::collections::HashSet::new();
        structural_eq_walk(self, a, b, &mut seen)
    }
}

fn mix_tag(state: &mut u64, tag: &[u8]) {
    *state ^= fnv1a64(tag);
    *state = state.wrapping_mul(FNV_PRIME);
}

fn mix_u64(state: &mut u64, v: u64) {
    *state ^= v;
    *state = state.wrapping_mul(FNV_PRIME);
}

fn structure_hash(node: &TermNode) -> u64 {
    let mut state = FNV_OFFSET_BASIS;
    match node {
        TermNode::Atom(atom) => {
            mix_tag(&mut state, b"atom");
            match atom {
                Atom::Number(n) => {
                    mix_tag(&mut state, b"num");
                    mix_u64(&mut state, n.fingerprint_domain_tag());
                    mix_u64(&mut state, fnv1a64(n.to_render_string().as_bytes()));
                }
                Atom::String(v) => {
                    mix_tag(&mut state, b"str");
                    mix_u64(&mut state, fnv1a64(v.as_bytes()));
                }
                Atom::Symbol(sym) => {
                    mix_tag(&mut state, b"sym");
                    mix_u64(&mut state, u64::from(sym.0));
                }
                Atom::Boolean(b) => {
                    mix_tag(&mut state, b"bool");
                    mix_u64(&mut state, u64::from(*b));
                }
                Atom::Null => mix_tag(&mut state, b"null"),
                Atom::Constant(c) => {
                    mix_tag(&mut state, b"const");
                    mix_u64(&mut state, u64::from(c.discriminant()));
                }
            }
        }
        TermNode::Collection { kind, elements } => {
            mix_tag(&mut state, b"collection");
            mix_u64(&mut state, collection_kind_tag(*kind));
            mix_u64(&mut state, elements.len() as u64);
            for id in elements {
                mix_u64(&mut state, u64::from(id.0));
            }
        }
        TermNode::Application { head, arguments } => {
            mix_tag(&mut state, b"app");
            match *head {
                ApplicationHead::Semantic(op) => {
                    mix_tag(&mut state, b"sem");
                    mix_u64(&mut state, u64::from(op.discriminant()));
                }
                ApplicationHead::Extension(op) => {
                    mix_tag(&mut state, b"ext");
                    mix_u64(&mut state, u64::from(op.0));
                }
            }
            mix_u64(&mut state, arguments.len() as u64);
            for id in arguments {
                mix_u64(&mut state, u64::from(id.0));
            }
        }
    }
    state
}

fn collection_kind_tag(kind: CollectionKind) -> u64 {
    match kind {
        CollectionKind::StructuralSequence => 1,
        CollectionKind::Tuple => 2,
        CollectionKind::OrderedCollection => 3,
        CollectionKind::SetLikeCollection => 4,
        CollectionKind::Vector => 5,
        CollectionKind::MatrixRow => 6,
        CollectionKind::MatrixColumn => 7,
        CollectionKind::Matrix => 8,
        CollectionKind::DomainCollection(id) => 0x1000 | u64::from(id.0),
    }
}

fn structural_eq_walk(arena: &TermStore, x: TermId, y: TermId, seen: &mut std::collections::HashSet<(u32, u32)>) -> bool {
    if x == y {
        return true;
    }
    if !seen.insert((x.0, y.0)) {
        return true;
    }
    match (arena.get(x), arena.get(y)) {
        (Some(TermNode::Atom(p)), Some(TermNode::Atom(q))) => p == q,
        (Some(TermNode::Collection { kind: kx, elements: xs }), Some(TermNode::Collection { kind: ky, elements: ys })) => {
            kx == ky && xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
        }
        (Some(TermNode::Application { head: op_x, arguments: xs }), Some(TermNode::Application { head: op_y, arguments: ys })) => {
            op_x == op_y && xs.len() == ys.len() && xs.iter().zip(ys.iter()).all(|(a, b)| structural_eq_walk(arena, *a, *b, seen))
        }
        _ => false,
    }
}

fn verify_term(arena: &TermStore, id: TermId, stack: &mut Vec<TermId>) -> Result<()> {
    if stack.contains(&id) {
        return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation));
    }
    let Some(kind) = arena.get(id)
    else {
        return Err(Diagnostic::new(DiagnosticCode::InvalidIndex));
    };
    stack.push(id);
    let result = match kind {
        TermNode::Atom(_) => Ok(()),
        TermNode::Collection { elements, .. } => {
            for child in elements {
                verify_term(arena, *child, stack)?;
            }
            Ok(())
        }
        TermNode::Application { arguments, .. } => {
            for child in arguments {
                verify_term(arena, *child, stack)?;
            }
            Ok(())
        }
    };
    stack.pop();
    result
}
