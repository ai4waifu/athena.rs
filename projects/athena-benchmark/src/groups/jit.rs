//! `jit` 分组：parity 门与 `athena-jit` 可用性。

use athena_jit::{JitAvailability, ParityOutcome, availability, polynomial_mul_parity};

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct JitAvailabilityFixture;

impl Fixture for JitAvailabilityFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("jit.availability", BenchGroup::Jit, "n/a", "jit")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let avail = availability();
        if matches!(avail, JitAvailability::Available) {
            return Err("native kernel not wired yet".into());
        }
        let parity = polynomial_mul_parity(|| 6i64, || None::<i64>);
        if !matches!(parity, ParityOutcome::JitUnavailable) {
            return Err(format!("expected JitUnavailable, got {parity:?}"));
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "athena-jit unavailable stub parity"))
    }

    fn run_once(&self) {
        let _ = availability();
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(JitAvailabilityFixture));
}
