//! `engine` 分组种子 fixture。

use athena_engine::{Session, interp::vm::evaluate_session};

fn push_int(n: i64, session: &mut Session) -> athena_types::TermId {
    athena_engine::arena_ops::push_int(session, n)
}

fn push_app_named(head: &str, args: Vec<athena_types::TermId>, session: &mut Session) -> athena_types::TermId {
    athena_engine::arena_ops::push_app_named(session, head, args)
}

use athena_engine::interp;

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
        let expr = push_app_named("Power", vec![push_int(2, &mut session), push_int(10, &mut session)], &mut session);
        let v = evaluate_session(&mut session, expr).term;
        let Some(x) = interp::number_of(&session, v).and_then(athena_engine::numeric::to_f64_lossy)
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
        let expr = push_app_named("Power", vec![push_int(3, &mut session), push_int(8, &mut session)], &mut session);
        let _ = evaluate_session(&mut session, expr);
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(EvalPowerFixture));
}
