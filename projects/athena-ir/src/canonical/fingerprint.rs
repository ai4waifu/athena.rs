//! IR term 规范结构 hash（缓存键 · wire · JIT kernel key）。
//!
//! FNV-1a 64 稳定实现：同结构同 hash，与插入顺序、进程无关。
//! 语义算子经 [`SemanticOperator::discriminant`](crate::SemanticOperator::discriminant) 参与 hash
//! （与 registry 无关）。扩展算子一律按 [`ExtensionOperatorId`](athena_types::ExtensionOperatorId)
//! 参与 hash（Session-local 身份）。**禁止**把 `display_name` 当作数学 / 缓存身份。

use athena_types::{CollectionKind, TermId};

use crate::{
    node::{Atom, TermNode},
    operator::ApplicationHead,
    store::TermStore,
};

/// FNV-1a 64 偏移基底。
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64 素数。
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

/// 对 term 子树求规范结构 hash（semantic discriminant · extension id）。
///
/// Extension 身份是 Session-local [`ExtensionOperatorId`](athena_types::ExtensionOperatorId)。
/// 跨 Session 稳定身份需要单独的注册协议，不能用显示名冒充。
pub fn canonical_hash(arena: &TermStore, root: TermId) -> u64 {
    hash_walk(arena, root).state
}

struct HashWalk<'a> {
    arena: &'a TermStore,
    state: u64,
    seen: Vec<TermId>,
}

fn hash_walk(arena: &TermStore, root: TermId) -> HashWalk<'_> {
    let mut s = HashWalk { arena, state: FNV_OFFSET_BASIS, seen: Vec::new() };
    hash_term(&mut s, root);
    s
}

fn hash_term(s: &mut HashWalk<'_>, id: TermId) {
    if s.seen.contains(&id) {
        mix_tag(&mut s.state, b"cycle");
        return;
    }
    let Some(kind) = s.arena.get(id) else {
        mix_tag(&mut s.state, b"invalid");
        return;
    };
    s.seen.push(id);
    match kind {
        TermNode::Atom(Atom::Number(n)) => {
            mix_tag(&mut s.state, b"num");
            mix_u64(&mut s.state, n.fingerprint_content_hash());
            mix_u64(&mut s.state, n.fingerprint_domain_tag());
        }
        TermNode::Atom(Atom::String(v)) => {
            mix_tag(&mut s.state, b"str");
            mix_u64(&mut s.state, fnv1a64(v.as_bytes()));
        }
        TermNode::Atom(Atom::Symbol(sym)) => {
            mix_tag(&mut s.state, b"sym");
            match s.arena.symbols().resolve(*sym) {
                Some(name) => mix_u64(&mut s.state, fnv1a64(name.as_bytes())),
                None => mix_u64(&mut s.state, u64::from(sym.0)),
            }
        }
        TermNode::Atom(Atom::Boolean(b)) => {
            mix_tag(&mut s.state, b"bool");
            mix_u64(&mut s.state, u64::from(*b));
        }
        TermNode::Atom(Atom::Null) => mix_tag(&mut s.state, b"null"),
        TermNode::Atom(Atom::Constant(c)) => {
            mix_tag(&mut s.state, b"const");
            mix_u64(&mut s.state, u64::from(c.discriminant()));
        }
        TermNode::Collection { kind, elements: items } => {
            mix_tag(&mut s.state, b"collection");
            mix_u64(&mut s.state, collection_kind_tag(*kind));
            mix_len(&mut s.state, items.len());
            for c in items {
                hash_term(s, *c);
            }
        }
        TermNode::Application { head, arguments: args } => {
            mix_tag(&mut s.state, b"app");
            match *head {
                ApplicationHead::Semantic(op) => {
                    mix_tag(&mut s.state, b"sem");
                    mix_u64(&mut s.state, u64::from(op.discriminant()));
                }
                ApplicationHead::Extension(op) => {
                    mix_tag(&mut s.state, b"ext");
                    mix_u64(&mut s.state, u64::from(op.0));
                }
            }
            mix_len(&mut s.state, args.len());
            for c in args {
                hash_term(s, *c);
            }
        }
    }
    s.seen.pop();
}
