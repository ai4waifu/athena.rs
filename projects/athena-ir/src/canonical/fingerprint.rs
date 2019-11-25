//! IR term 规范结构 hash（缓存键 · wire · JIT kernel key）。
//!
//! FNV-1a 64 稳定实现：同结构同 hash，与插入顺序、进程无关。
//! Semantic ops hash via [`SemanticOperator::discriminant`](crate::SemanticOperator::discriminant)
//! (registry-independent). Extension ops hash by id, or by display name when using
//! [`canonical_hash_named`].

use athena_types::{CollectionKind, TermId};

use crate::{
    node::{Atom, TermNode},
    operator::{ApplicationHead, ExtensionRegistry},
    store::TermStore,
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
pub fn canonical_hash(arena: &TermStore, root: TermId) -> u64 {
    hash_walk(arena, None, root).state
}

/// Cross-registry stable hash: semantic via discriminant, extension via display name.
pub fn canonical_hash_named(arena: &TermStore, registry: &ExtensionRegistry, root: TermId) -> u64 {
    hash_walk(arena, Some(registry), root).state
}

struct HashWalk<'a> {
    arena: &'a TermStore,
    registry: Option<&'a ExtensionRegistry>,
    state: u64,
    seen: Vec<TermId>,
}

fn hash_walk<'a>(arena: &'a TermStore, registry: Option<&'a ExtensionRegistry>, root: TermId) -> HashWalk<'a> {
    let mut s = HashWalk { arena, registry, state: FNV_OFFSET_BASIS, seen: Vec::new() };
    hash_term(&mut s, root);
    s
}

fn hash_term(s: &mut HashWalk<'_>, id: TermId) {
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
        TermNode::Atom(Atom::Number(n)) => {
            mix_tag(&mut s.state, b"num");
            mix_u64(&mut s.state, fnv1a64(n.to_render_string().as_bytes()));
            mix_u64(&mut s.state, fnv1a64(format!("{:?}", n.domain()).as_bytes()));
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
                    match s.registry.and_then(|r| r.display_name(op)) {
                        Some(name) => mix_u64(&mut s.state, fnv1a64(name.as_bytes())),
                        None => mix_u64(&mut s.state, u64::from(op.0)),
                    }
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
