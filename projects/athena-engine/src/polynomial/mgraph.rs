//! Session ↔ M-Graph 多项式集成。

use athena_types::Diagnostic;

use super::{
    cache_key::cache_key_for_request,
    request::PolynomialRequest,
    result::{PolynomialResult, execute_polynomial_with_rings},
    ring_table::RingTable,
    PolynomialCacheKey,
};
use crate::mgraph::{MGraphState, PolynomialCacheEntry, witness_from_exact};

/// 在 M-Graph 上下文中执行多项式请求（缓存 + witness）。
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
    if let PolynomialResult::Exact { ref value } = result {
        let witness = witness_from_exact(&key, value);
        let witness_edge = state.polynomial.insert(PolynomialCacheEntry {
            key: key.clone(),
            result: result.clone(),
            witness,
        });
        state.witnesses.push(witness_edge);
    }
    result
}

/// 将已有精确结果写入 M-Graph（测试 / 外部 orchestrator）。
pub fn record_polynomial_result(
    key: PolynomialCacheKey,
    result: PolynomialResult,
    state: &mut MGraphState,
) -> Result<(), Diagnostic> {
    match &result {
        PolynomialResult::Exact { value } => {
            let witness = witness_from_exact(&key, value);
            let edge = state.polynomial.insert(PolynomialCacheEntry { key, result, witness });
            state.witnesses.push(edge);
            Ok(())
        }
        PolynomialResult::Unevaluated { reason } => Err(reason.clone()),
    }
}
