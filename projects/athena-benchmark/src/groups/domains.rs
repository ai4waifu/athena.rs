//! `domains` 分组种子 fixture（`sample_1d`）。

use athena_engine::{SampleDomain, SamplingPolicy, Term, sample_1d};

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct Sample1dFixture;

impl Fixture for Sample1dFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta { id: "domains.sample_1d_square", group: BenchGroup::Domains, scale: "samples_17", domain: "sampled_curve" }
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]);
        let curve = sample_1d(&expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(5))
            .map_err(|d| d.code.as_str().to_string())?;
        if curve.points.len() != 5 {
            return Err(format!("expected 5 points, got {}", curve.points.len()));
        }
        if (curve.points[0].y - 1.0).abs() > 1e-9 {
            return Err("sample_1d endpoint mismatch".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Machine, DeterminacyKind::Deterministic, "sample_1d x^2 on [-1,1]"))
    }

    fn run_once(&self) {
        let expr = Term::apply("Power", vec![Term::symbol("x"), Term::int(2)]);
        let _ = sample_1d(&expr, "x", SampleDomain::new(-1.0, 1.0), SamplingPolicy::samples(17));
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(Sample1dFixture));
}
