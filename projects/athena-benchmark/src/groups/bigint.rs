//! `bigint` group: unified matrix registered into `athena-bench`.

use crate::{
    bigint::{BenchCase, BigIntPrepared, prepare_all},
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct BigIntFixture {
    prepared: BigIntPrepared,
    id: &'static str,
    scale: &'static str,
}

impl BigIntFixture {
    fn new(prepared: BigIntPrepared) -> Self {
        let case = prepared.case();
        let id = leak(case.id());
        let scale = leak(format!("{}bit", case.bits));
        Self { prepared, id, scale }
    }
}

impl Fixture for BigIntFixture {
    fn meta(&self) -> FixtureMeta {
        let case = self.prepared.case();
        FixtureMeta {
            id: self.id,
            group: BenchGroup::Bigint,
            scale: self.scale,
            domain: "exact_integer",
            layer: Some(case.layer),
            context_policy: Some(case.context_policy),
            implementation: Some(case.implementation.as_str()),
            operation: Some(case.operation.as_str()),
            bits: Some(case.bits),
            gc_mode: Some(case.gc_mode()),
        }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        self.prepared.validate()?;
        Ok(ValidationSummary::passed(
            ExactnessKind::Exact,
            DeterminacyKind::Deterministic,
            "bigint matrix vs athena numeric reference",
        ))
    }

    fn run_once(&self) {
        self.prepared.run_once();
    }
}

pub(super) fn register(suite: &mut Suite) {
    for prepared in prepare_all() {
        let _case: BenchCase = prepared.case();
        suite.register(Box::new(BigIntFixture::new(prepared)));
    }
}

fn leak(s: String) -> &'static str {
    Box::leak(s.into_boxed_str())
}
