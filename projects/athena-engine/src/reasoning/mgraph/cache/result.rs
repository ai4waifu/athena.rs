//! 非语义结果缓存（可驱逐；不得写入 semantic core）。

use crate::{
    domains::polynomial::{PolynomialCacheKey, PolynomialResult},
    reasoning::mgraph::{
        admission::gate::AdmissionOutcome,
        core::types::RewriteWitness,
        polynomial::{PolynomialCacheEntry, PolynomialCacheTier, PolynomialMGraphStore, witness_from_exact},
    },
};

/// 操作层结果缓存（非数学真相源）。
///
/// **不**实现 [`Clone`]（多项式缓存含 owning 载荷）。
#[derive(Debug, Default, PartialEq)]
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
    ) -> Option<RewriteWitness> {
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
