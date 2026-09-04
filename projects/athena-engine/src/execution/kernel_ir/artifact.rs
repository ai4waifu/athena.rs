//! KernelIR 合同：已验证子图 → 执行计划摘要（非第二套数学 IR）。

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

use crate::{
    domains::polynomial::PolynomialCacheOp,
    reasoning::mgraph::{FactLog, POLYNOMIAL_PROVIDER_ID, Proposition},
};

/// 单条 kernel 操作（当前覆盖多项式域）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KernelOperation {
    /// 多项式精确运算。
    Polynomial {
        /// 缓存操作标签。
        operation: PolynomialCacheOp,
        /// 请求 stable 指纹。
        request_fingerprint: u64,
    },
}

/// 已验证稳定子图的执行计划摘要。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelIR {
    /// 计划 stable hash。
    pub fingerprint: u64,
    /// 有序操作列表。
    pub operations: Vec<KernelOperation>,
    /// 产出 capability provider（当前仅多项式）。
    pub provider: crate::reasoning::mgraph::CapabilityProviderId,
}

impl KernelIR {
    /// 空计划。
    pub fn empty() -> Self {
        Self { fingerprint: 0, operations: Vec::new(), provider: POLYNOMIAL_PROVIDER_ID }
    }

    /// 由 fact log 抽取已验证多项式 claim 构造 KernelIR。
    pub fn extract_from_fact_log(fact_log: &FactLog) -> Self {
        let mut operations = Vec::new();
        for claim in fact_log.claims() {
            if let Proposition::PolynomialResult { operation, request_fingerprint } = &claim.claim.proposition {
                operations
                    .push(KernelOperation::Polynomial { operation: *operation, request_fingerprint: *request_fingerprint });
            }
        }
        let fingerprint = hash_operations(&operations);
        Self { fingerprint, operations, provider: POLYNOMIAL_PROVIDER_ID }
    }
}

fn hash_operations(ops: &[KernelOperation]) -> u64 {
    let mut h = DefaultHasher::new();
    ops.hash(&mut h);
    h.finish()
}
