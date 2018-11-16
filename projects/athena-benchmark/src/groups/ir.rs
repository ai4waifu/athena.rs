//! `ir` 分组种子 fixture。

use athena_ir::{TermArena, TermBuilder, canonical_hash};
use athena_numeric::Number;
use athena_types::SourceSpan;

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct ArenaHashFixture;

impl Fixture for ArenaHashFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta { id: "ir.arena_canonical_hash", group: BenchGroup::Ir, scale: "small_app_tree", domain: "core_ir" }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let (h1, root) = build_and_hash();
        let mut arena = TermArena::new();
        let mut b = TermBuilder::new(&mut arena);
        let span = SourceSpan::default();
        let a = b.number(Number::small_int(1), span);
        let c = b.number(Number::small_int(2), span);
        let root2 = b.list(vec![a, c], span);
        arena.verify(root2).map_err(|d| d.code.as_str().to_string())?;
        let h2 = canonical_hash(&arena, root2);
        if h1 != h2 {
            return Err(format!("canonical hash mismatch: {h1:#x} vs {h2:#x}"));
        }
        let _ = root;
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "verify + canonical_hash stable"))
    }

    fn run_once(&self) {
        let _ = build_and_hash();
    }
}

fn build_and_hash() -> (u64, athena_types::TermId) {
    let mut arena = TermArena::new();
    let mut b = TermBuilder::new(&mut arena);
    let span = SourceSpan::default();
    let a = b.number(Number::small_int(1), span);
    let c = b.number(Number::small_int(2), span);
    let root = b.list(vec![a, c], span);
    let _ = arena.verify(root);
    (canonical_hash(&arena, root), root)
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(ArenaHashFixture));
}
