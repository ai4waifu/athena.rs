//! Athena 内核基准框架：确定性 fixture、校验与机器可读合同报告。
//!
//! - **合同 / 资源**：`athena-bench`（本 crate 的 bin）
//! - **性能 ns/op**：Criterion（`cargo bench`），与本 crate 共享 [`bigint`] fixture
//!
//! 禁止在本 crate 内再用 `Instant` 自造微基准计时器。

#![deny(missing_docs)]

pub mod bigint;
pub mod env;
pub mod fixture;
pub mod groups;
pub mod report;
pub mod validate;

pub use env::BenchEnv;
pub use fixture::{BenchGroup, Fixture, FixtureMeta, RunConfig, Suite, SuiteError};
pub use report::{FixtureReport, Report, ReportTier};
pub use validate::{DeterminacyKind, ExactnessKind, ValidationSummary};

use crate::fixture::run_fixture;

/// 运行已注册 suite 中与 `groups` 匹配的 fixture，产出合同 / 资源报告（无 ns/op）。
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
