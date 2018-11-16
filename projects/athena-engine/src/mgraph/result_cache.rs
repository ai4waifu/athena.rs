//! 非语义结果缓存（可驱逐；不得写入 semantic core）。

use super::{
    admission::AdmissionOutcome,
    polynomial::{PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, witness_from_exact},
};
use crate::polynomial::{PolynomialCacheKey, PolynomialResult};

/// 操作层结果缓存（非数学真相源）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ResultCache {
    /// 多项式域结果缓存。
    pub polynomial: PolynomialMGraphStore,
}

impl ResultCache {
    /// 写入多项式结果；admission 决定 verified / partial 层。
    pub fn store_polynomial(
        &mut self,
        key: PolynomialCacheKey,
        result: PolynomialResult,
        outcome: &AdmissionOutcome,
    ) -> Option<crate::mgraph::RewriteWitness> {
        let (tier, witness) = match (outcome, &result) {
            (AdmissionOutcome::Admitted(_), PolynomialResult::Exact { value }) => {
                (PolynomialCacheTier::Verified, Some(witness_from_exact(&key, value)))
            }
            (AdmissionOutcome::Rejected { .. }, PolynomialResult::Exact { .. }) => (PolynomialCacheTier::Partial, None),
            _ => (PolynomialCacheTier::Partial, None),
        };
        self.polynomial.insert(PolynomialCacheEntry { key, result, tier, witness })
    }
}
