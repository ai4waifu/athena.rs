//! `engine` 分组种子 fixture。

use athena_engine::{Term, evaluate};

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

struct EvalPowerFixture;

impl Fixture for EvalPowerFixture {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("engine.eval_power", BenchGroup::Engine, "small_term", "machine_real")
    }

    fn validate(&self) -> Result<ValidationSummary, String> {
        let v = evaluate(&Term::apply("Power", vec![Term::int(2), Term::int(10)]));
        let Some(x) = v.as_f64_lossy()
        else {
            return Err("eval did not yield f64".into());
        };
        if (x - 1024.0).abs() > 1e-9 {
            return Err(format!("expected 1024, got {x}"));
        }
        Ok(ValidationSummary::passed(ExactnessKind::Mixed, DeterminacyKind::Deterministic, "Power(2,10)=1024"))
    }

    fn run_once(&self) {
        let _ = evaluate(&Term::apply("Power", vec![Term::int(3), Term::int(8)]));
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(EvalPowerFixture));
}
