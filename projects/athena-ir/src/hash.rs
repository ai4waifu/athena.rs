//! IR term 规范结构 hash（wire / 缓存键）。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use athena_types::TermId;

use crate::{
    arena::TermArena,
    node::{AtomKind, TermKind},
};

/// 对 term 子树求 hash（结构 + 载荷稳定）。
pub fn canonical_hash(arena: &TermArena, root: TermId) -> u64 {
    let mut h = DefaultHasher::new();
    hash_term(arena, root, &mut h, &mut vec![]);
    h.finish()
}

fn hash_term(arena: &TermArena, id: TermId, h: &mut DefaultHasher, seen: &mut Vec<TermId>) {
    if seen.contains(&id) {
        "cycle".hash(h);
        return;
    }
    seen.push(id);
    let Some(kind) = arena.get(id)
    else {
        "invalid".hash(h);
        seen.pop();
        return;
    };
    match kind {
        TermKind::Atom(AtomKind::Number(n)) => {
            "num".hash(h);
            format!("{n:?}").hash(h);
        }
        TermKind::Atom(AtomKind::String(s)) => {
            "str".hash(h);
            s.hash(h);
        }
        TermKind::Atom(AtomKind::Symbol(sym)) => {
            "sym".hash(h);
            sym.0.hash(h);
            if let Some(name) = arena.symbols().resolve(*sym) {
                name.hash(h);
            }
        }
        TermKind::List(items) => {
            "list".hash(h);
            for c in items {
                hash_term(arena, *c, h, seen);
            }
        }
        TermKind::App { op, args } => {
            "app".hash(h);
            op.0.hash(h);
            for c in args {
                hash_term(arena, *c, h, seen);
            }
        }
    }
    seen.pop();
}
