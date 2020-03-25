//! 自 `src/domains/solve/frontier.rs` 迁出的原内联测试。

use athena_engine::{
    domains::solve::{ResumeKind, ResumeToken},
    runtime::results::{ResultProviderId, ResultProviderStamp},
};

#[test]
fn resume_rejects_missing_provider_stamp() {
    let token = ResumeToken::empty(ResumeKind::Cut);
    assert!(!token.accepts_provider(ResultProviderId::POLYNOMIAL.stamped()));
}

#[test]
fn resume_accepts_matching_provider_stamp() {
    let stamp = ResultProviderId::LINEAR_ALGEBRA.stamped();
    let token = ResumeToken::empty_with_provider(ResumeKind::LinearExact, stamp);
    assert!(token.accepts_provider(stamp));
}

#[test]
fn resume_rejects_stale_provider_version() {
    let stamp = ResultProviderId::NUMBER_THEORY.stamped();
    let token = ResumeToken::empty_with_provider(ResumeKind::Cut, stamp);
    let stale = ResultProviderStamp { id: ResultProviderId::NUMBER_THEORY, version: 0 };
    assert!(!token.accepts_provider(stale));
}
