//! 多项式 M-Graph 存储与 witness。

use std::collections::HashMap;

use super::types::{RewriteWitness, SolverId};

use crate::polynomial::{
    PolynomialCacheKey, PolynomialCacheOp, PolynomialDomainValue, PolynomialResult,
};

/// 多项式域 solver id（M-Graph / solver 共享）。
pub const POLYNOMIAL_SOLVER_ID: SolverId = SolverId(10);

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
#[derive(Debug, Clone, PartialEq)]
pub struct PolynomialCacheEntry {
    /// 缓存键。
    pub key: PolynomialCacheKey,
    /// 求值结果。
    pub result: PolynomialResult,
    /// witness。
    pub witness: PolynomialWitness,
}

/// M-Graph 内多项式子图状态。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PolynomialMGraphStore {
    /// 精确结果缓存（键 → 项）。
    pub entries: HashMap<PolynomialCacheKey, PolynomialCacheEntry>,
}

impl PolynomialMGraphStore {
    /// 查缓存。
    pub fn get(&self, key: &PolynomialCacheKey) -> Option<&PolynomialCacheEntry> {
        self.entries.get(key)
    }

    /// 写入缓存并返回 rewrite witness。
    pub fn insert(&mut self, entry: PolynomialCacheEntry) -> RewriteWitness {
        self.entries.insert(entry.key.clone(), entry);
        RewriteWitness {
            solver: POLYNOMIAL_SOLVER_ID,
            inputs: Vec::new(),
            outputs: Vec::new(),
        }
    }

    /// 已缓存条目数。
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// 从 [`PolynomialResult`] 构造 witness 摘要。
pub fn witness_from_exact(key: &PolynomialCacheKey, value: &PolynomialDomainValue) -> PolynomialWitness {
    let (output_summary, groebner_steps) = match value {
        PolynomialDomainValue::Polynomial(v) => (format!("poly:{}", v.inner.terms.len()), None),
        PolynomialDomainValue::GroebnerBasis(v) => (
            format!("gb:{}:{}", v.basis.len(), v.certificate.s_pair_steps),
            Some(v.certificate.s_pair_steps),
        ),
        PolynomialDomainValue::Placeholder => ("placeholder".into(), None),
    };
    PolynomialWitness {
        operation: key.operation,
        input_hashes: key.input_hashes.clone(),
        output_summary,
        groebner_steps,
    }
}
