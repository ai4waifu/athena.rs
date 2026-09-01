//! `jit` 分组：无 `athena-jit` 时跳过。

use crate::fixture::{BenchGroup, Fixture, FixtureMeta, Suite};
use crate::validate::{DeterminacyKind, ExactnessKind, ValidationSummary};

struct JitPlaceholderFixture;

impl Fixture for JitPlaceholderFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta {
            id: "jit.placeholder",
            group: BenchGroup::Jit,
            scale: "n/a",
            domain: "jit",
        }
    }

    fn skip_reason(&self) -> Option<&'static str> {
        if cfg!(feature = "jit") {
            Some("athena-jit not wired yet")
        } else {
            Some("jit feature disabled")
        }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        Ok(ValidationSummary::passed(
            ExactnessKind::Unspecified,
            DeterminacyKind::Unspecified,
            "jit placeholder",
        ))
    }

    fn run_once(&self) {}
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(JitPlaceholderFixture));
}
