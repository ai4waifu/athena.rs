//! `rewriter` 分组种子 fixture。

use athena_ir::{TermArena, TermBuilder};
use athena_numeric::Number;
use athena_rewriter::{RewriteOptions, Rewriter};
use athena_types::SourceSpan;

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct SimplifyStubFixture;

impl Fixture for SimplifyStubFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta { id: "rewriter.simplify_noop", group: BenchGroup::Rewriter, scale: "single_atom", domain: "core_ir" }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let mut arena = TermArena::new();
        let mut b = TermBuilder::new(&mut arena);
        let root = b.number(Number::small_int(42), SourceSpan::default());
        let rw = Rewriter::with_options(RewriteOptions { constant_fold: true });
        let out = rw.simplify(&mut arena, root).map_err(|d| d.code.as_str().to_string())?;
        if out.root != root {
            return Err("simplify changed root unexpectedly".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "rewriter.simplify on atom"))
    }

    fn run_once(&self) {
        let mut arena = TermArena::new();
        let mut b = TermBuilder::new(&mut arena);
        let root = b.number(Number::small_int(7), SourceSpan::default());
        let rw = Rewriter::with_options(RewriteOptions { constant_fold: true });
        let _ = rw.simplify(&mut arena, root);
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(SimplifyStubFixture));
}
