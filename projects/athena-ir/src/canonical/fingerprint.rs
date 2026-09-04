//! IR term 规范结构 hash（缓存键 · wire · JIT kernel key）。
//!
//! FNV-1a 64 稳定实现：同结构同 hash，与插入顺序、进程无关。
//! 算子默认按 [`OperatorId`](athena_types::OperatorId)（session 内稳定）；
//! 跨注册表稳定键用 [`canonical_hash_named`]（按注册名）。

use athena_types::ExprId;

use crate::{
    arena::ExprArena,
    node::{Atom, ExprNode},
    operator::OperatorRegistry,
};

/// FNV-1a 64 offset basis。
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64 prime。
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a 64 字节流 hash（IR 与领域指纹共用基元）。
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h = FNV_OFFSET_BASIS;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(FNV_PRIME);
    }
    h
}

fn mix_tag(state: &mut u64, tag: &[u8]) {
    *state ^= fnv1a64(tag);
    *state = state.wrapping_mul(FNV_PRIME);
}

fn mix_u64(state: &mut u64, v: u64) {
    *state ^= v;
    *state = state.wrapping_mul(FNV_PRIME);
}

fn mix_len(state: &mut u64, len: usize) {
    mix_u64(state, len as u64);
}

/// 对 term 子树求规范结构 hash（算子按 [`OperatorId`]）。
pub fn canonical_hash(arena: &ExprArena, root: ExprId) -> u64 {
    hash_walk(arena, None, root).state
}

/// 对 term 子树求跨注册表稳定 hash：算子按注册名（JIT kernel key / wire）。
pub fn canonical_hash_named(arena: &ExprArena, registry: &OperatorRegistry, root: ExprId) -> u64 {
    hash_walk(arena, Some(registry), root).state
}

struct HashWalk<'a> {
    arena: &'a ExprArena,
    registry: Option<&'a OperatorRegistry>,
    state: u64,
    seen: Vec<ExprId>,
}

fn hash_walk<'a>(arena: &'a ExprArena, registry: Option<&'a OperatorRegistry>, root: ExprId) -> HashWalk<'a> {
    let mut s = HashWalk { arena, registry, state: FNV_OFFSET_BASIS, seen: Vec::new() };
    hash_term(&mut s, root);
    s
}

fn hash_term(s: &mut HashWalk<'_>, id: ExprId) {
    if s.seen.contains(&id) {
        mix_tag(&mut s.state, b"cycle");
        return;
    }
    let Some(kind) = s.arena.get(id)
    else {
        mix_tag(&mut s.state, b"invalid");
        return;
    };
    s.seen.push(id);
    match kind {
        ExprNode::Atom(Atom::Number(n)) => {
            mix_tag(&mut s.state, b"num");
            mix_u64(&mut s.state, fnv1a64(n.to_render_string().as_bytes()));
            mix_u64(&mut s.state, fnv1a64(format!("{:?}", n.domain()).as_bytes()));
        }
        ExprNode::Atom(Atom::String(v)) => {
            mix_tag(&mut s.state, b"str");
            mix_u64(&mut s.state, fnv1a64(v.as_bytes()));
        }
        ExprNode::Atom(Atom::Symbol(sym)) => {
            mix_tag(&mut s.state, b"sym");
            match s.arena.symbols().resolve(*sym) {
                Some(name) => mix_u64(&mut s.state, fnv1a64(name.as_bytes())),
                None => mix_u64(&mut s.state, u64::from(sym.0)),
            }
        }
        ExprNode::Atom(Atom::Boolean(b)) => {
            mix_tag(&mut s.state, b"bool");
            mix_u64(&mut s.state, u64::from(*b));
        }
        ExprNode::Atom(Atom::Null) => mix_tag(&mut s.state, b"null"),
        ExprNode::List(items) => {
            mix_tag(&mut s.state, b"list");
            mix_len(&mut s.state, items.len());
            for c in items {
                hash_term(s, *c);
            }
        }
        ExprNode::App { op, args } => {
            mix_tag(&mut s.state, b"app");
            match s.registry.and_then(|r| r.name(*op)) {
                Some(name) => mix_u64(&mut s.state, fnv1a64(name.as_bytes())),
                None => mix_u64(&mut s.state, u64::from(op.0)),
            }
            mix_len(&mut s.state, args.len());
            for c in args {
                hash_term(s, *c);
            }
        }
    }
    s.seen.pop();
}
