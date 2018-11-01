//! Session ↔ M-Graph 多项式集成。

use athena_types::Diagnostic;

use super::{
    cache_key::cache_key_for_request,
    request::PolynomialRequest,
    result::{PolynomialResult, execute_polynomial_with_rings},
    ring_table::RingTable,
    PolynomialCacheKey,
};
use crate::mgraph::{
    AdmissionOutcome, MGraphState, PolynomialCacheEntry, PolynomialCacheTier, admit_polynomial_result, witness_from_exact,
};

/// 在 M-Graph 上下文中执行多项式请求（缓存 + admission gate）。
pub fn execute_polynomial_mgraph(
    request: PolynomialRequest,
    rings: &RingTable,
    state: &mut MGraphState,
) -> PolynomialResult {
    let key = match cache_key_for_request(&request, rings) {
        Ok(k) => k,
        Err(reason) => return PolynomialResult::Unevaluated { reason },
    };
    if let Some(entry) = state.polynomial.get(&key) {
        return entry.result.clone();
    }
    let result = execute_polynomial_with_rings(request, rings);
    record_polynomial_cache(key, result.clone(), state);
    result
}

/// 将已有结果写入 M-Graph（测试 / 外部 orchestrator）。
pub fn record_polynomial_result(
    key: PolynomialCacheKey,
    result: PolynomialResult,
    state: &mut MGraphState,
) -> Result<(), Diagnostic> {
    match &result {
        PolynomialResult::Exact { .. } => {
            record_polynomial_cache(key, result, state);
            Ok(())
        }
        PolynomialResult::Unevaluated { reason } => Err(reason.clone()),
    }
}

fn record_polynomial_cache(key: PolynomialCacheKey, result: PolynomialResult, state: &mut MGraphState) {
    let admission = admit_polynomial_result(&key, &result);
    let (tier, witness) = match (&admission, &result) {
        (AdmissionOutcome::Admitted(vc), PolynomialResult::Exact { value }) => {
            state.verified_claims.push(vc.clone());
            (
                PolynomialCacheTier::Verified,
                Some(witness_from_exact(&key, value)),
            )
        }
        (AdmissionOutcome::Rejected { .. }, PolynomialResult::Exact { .. }) => {
            (PolynomialCacheTier::Partial, None)
        }
        _ => (PolynomialCacheTier::Partial, None),
    };
    if let Some(edge) = state.polynomial.insert(PolynomialCacheEntry { key, result, tier, witness }) {
        state.witnesses.push(edge);
    }
}
