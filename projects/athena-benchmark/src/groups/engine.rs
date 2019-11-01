//! `engine` 分组种子 fixture。

use athena_engine::{
    execution::{self, evaluate_term},
    runtime::{
        Session,
        values::arena::{push_int, push_semantic},
    },
};
use athena_ir::SemanticOperator;

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
        let mut session = Session::new();
        let two = push_int(&mut session, 2);
        let ten = push_int(&mut session, 10);
        let expr = push_semantic(&mut session, SemanticOperator::Power, vec![two, ten]);
        let v = evaluate_term(&mut session, expr).term;
        let Some(x) = execution::number_of(&session, v).and_then(athena_numeric::to_f64_lossy)
        else {
            return Err("eval did not yield f64".into());
        };
        if (x - 1024.0).abs() > 1e-9 {
            return Err(format!("expected 1024, got {x}"));
        }
        Ok(ValidationSummary::passed(ExactnessKind::Mixed, DeterminacyKind::Deterministic, "Power(2,10)=1024"))
    }

    fn run_once(&self) {
        let mut session = Session::new();
        let three = push_int(&mut session, 3);
        let eight = push_int(&mut session, 8);
        let expr = push_semantic(&mut session, SemanticOperator::Power, vec![three, eight]);
        let _ = evaluate_term(&mut session, expr);
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(EvalPowerFixture));
}
