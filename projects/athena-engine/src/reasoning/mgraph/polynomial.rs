//! 多项式 M-Graph 存储与 witness。

use std::collections::HashMap;

use crate::reasoning::mgraph::core::types::{CapabilityProviderId, RewriteWitness};

use crate::domains::polynomial::{PolynomialCacheKey, PolynomialCacheOp, PolynomialDomainValue, PolynomialResult};

/// 多项式域 capability provider 身份。
pub const POLYNOMIAL_PROVIDER_ID: CapabilityProviderId = CapabilityProviderId(10);

/// 缓存接纳层级（semantic core 与 partial 结果分离）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolynomialCacheTier {
    /// 已通过 admission gate，可关联 witness / verified claim。
    Verified,
    /// 已缓存但未接纳（partial / placeholder / incomplete Gröbner）。
    Partial,
}

/// 多项式运算 witness（可验证 metadata）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolynomialWitness {
    /// 操作。
    pub operation: PolynomialCacheOp,
    /// 输入 canonical hash。
    pub input_hashes: Vec<u64>,
    /// 结果摘要（basis 长度或 render 指纹）。
    pub output_summary: String,
    /// Gröbner S-pair 步数（若有）。
    pub groebner_steps: Option<u32>,
}

/// 单条缓存项。
///
/// Living `31`：**不**实现 [`Clone`]（结果含多项式 owning 载荷）。
#[derive(Debug, PartialEq)]
pub struct PolynomialCacheEntry {
    /// 缓存键。
    pub key: PolynomialCacheKey,
    /// 求值结果。
    pub result: PolynomialResult,
    /// 接纳层级。
    pub tier: PolynomialCacheTier,
    /// witness（仅 [`PolynomialCacheTier::Verified`] 时有值）。
    pub witness: Option<PolynomialWitness>,
}

/// M-Graph 内多项式子图状态。
///
/// Living `31`：**不**实现 [`Clone`]。
#[derive(Debug, Default, PartialEq)]
pub struct PolynomialMGraphStore {
    /// 已接纳结果（semantic core 关联层）。
    verified: HashMap<PolynomialCacheKey, PolynomialCacheEntry>,
    /// 未接纳但可复用的 partial 结果。
    partial: HashMap<PolynomialCacheKey, PolynomialCacheEntry>,
}

impl PolynomialMGraphStore {
    /// 查缓存（verified 优先，其次 partial）。
    pub fn get(&self, key: &PolynomialCacheKey) -> Option<&PolynomialCacheEntry> {
        self.verified.get(key).or_else(|| self.partial.get(key))
    }

    /// 查 verified 层。
    pub fn get_verified(&self, key: &PolynomialCacheKey) -> Option<&PolynomialCacheEntry> {
        self.verified.get(key)
    }

    /// 查 partial 层。
    pub fn get_partial(&self, key: &PolynomialCacheKey) -> Option<&PolynomialCacheEntry> {
        self.partial.get(key)
    }

    /// 写入缓存；verified 层且含 witness 时返回 rewrite witness。
    pub fn insert(&mut self, entry: PolynomialCacheEntry) -> Option<RewriteWitness> {
        let edge = if entry.tier == PolynomialCacheTier::Verified {
            entry.witness.as_ref().map(|_| RewriteWitness { provider: POLYNOMIAL_PROVIDER_ID, inputs: Vec::new(), outputs: Vec::new() })
        }
        else {
            debug_assert!(entry.witness.is_none());
            None
        };
        let key = entry.key.owning_copy();
        match entry.tier {
            PolynomialCacheTier::Verified => {
                self.partial.remove(&key);
                self.verified.insert(key, entry);
            }
            PolynomialCacheTier::Partial => {
                self.verified.remove(&key);
                self.partial.insert(key, entry);
            }
        }
        edge
    }

    /// 按 request fingerprint 查找（verified 优先）。
    pub fn get_by_request_fingerprint(&self, request_fingerprint: u64) -> Option<&PolynomialCacheEntry> {
        self.verified.values().chain(self.partial.values()).find(|entry| entry.key.fingerprint() == request_fingerprint)
    }

    /// 总缓存条目数。
    pub fn len(&self) -> usize {
        self.verified.len() + self.partial.len()
    }

    /// verified 层条目数。
    pub fn verified_len(&self) -> usize {
        self.verified.len()
    }

    /// partial 层条目数。
    pub fn partial_len(&self) -> usize {
        self.partial.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.verified.is_empty() && self.partial.is_empty()
    }
}

/// 从已接纳的 exact 值构造 witness 摘要（调用方须保证非 Placeholder 且 Gröbner verified）。
pub fn witness_from_exact(key: &PolynomialCacheKey, value: &PolynomialDomainValue) -> PolynomialWitness {
    debug_assert!(!matches!(value, PolynomialDomainValue::Placeholder), "placeholder must not produce exact witness");
    debug_assert!(
        !matches!(value, PolynomialDomainValue::GroebnerBasis(v) if !v.is_exact_witness()),
        "unverified / incomplete Gröbner must not produce exact witness"
    );
    debug_assert!(
        !matches!(value, PolynomialDomainValue::Factorization(v) if !v.is_exact_witness()),
        "incomplete polynomial factorization must not produce exact witness"
    );
    let (output_summary, groebner_steps) = match value {
        PolynomialDomainValue::Polynomial(v) => (format!("poly:{}", v.inner.terms().len()), None),
        PolynomialDomainValue::GroebnerBasis(v) => {
            (format!("gb:{}:{}", v.basis.len(), v.certificate.s_pair_steps), Some(v.certificate.s_pair_steps))
        }
        PolynomialDomainValue::UnivariateDivision(v) => {
            (format!("div:q{}:r{}", v.quotient.inner.terms().len(), v.remainder.inner.terms().len()), None)
        }
        PolynomialDomainValue::Factorization(v) => (format!("factor:{}:{:?}", v.factors.len(), v.completeness()), None),
        PolynomialDomainValue::ModularImage(v) => (format!("modimg:{}:vanished={}", v.image.terms().len(), v.vanished), None),
        PolynomialDomainValue::Placeholder => ("placeholder".into(), None),
    };
    PolynomialWitness { operation: key.operation, input_hashes: key.input_hashes.clone(), output_summary, groebner_steps }
}
