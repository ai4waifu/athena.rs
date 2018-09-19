//! Athena 内核基准框架：确定性 fixture、校验与机器可读报告。
//!
//! 外部软件对照不属于本 crate。默认 pure-Rust eager；`jit` feature 仅占位。

#![deny(missing_docs)]

pub mod env;
pub mod fixture;
pub mod groups;
pub mod report;
pub mod timing;
pub mod validate;

pub use env::BenchEnv;
pub use fixture::{BenchGroup, Fixture, FixtureMeta, RunConfig, Suite, SuiteError};
pub use report::{FixtureReport, Report};
pub use timing::{TimingStats, measure};
pub use validate::{DeterminacyKind, ExactnessKind, ValidationSummary};

use crate::fixture::run_fixture;

/// 运行已注册 suite 中与 `groups` 匹配的 fixture，产出完整报告。
pub fn run_suite(suite: &Suite, config: &RunConfig) -> Result<Report, SuiteError> {
    let env = BenchEnv::capture(cfg!(feature = "jit"));
    let mut fixtures = Vec::new();
    for fixture in suite.fixtures() {
        let meta = fixture.meta();
        if !config.groups.is_empty() && !config.groups.iter().any(|g| *g == meta.group) {
            continue;
        }
        fixtures.push(run_fixture(fixture.as_ref(), config, &env)?);
    }
    Ok(Report { env, fixtures })
}
