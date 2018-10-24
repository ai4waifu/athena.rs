//! M-Graph 多项式缓存键（operation · canonical input hash · 环身份）。

use std::hash::{Hash, Hasher};

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    expr::Polynomial,
    groebner::GroebnerLimits,
    hash::canonical_hash as polynomial_canonical_hash,
    request::PolynomialRequest,
    ring_table::RingTable,
};

/// 缓存操作标签。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolynomialCacheOp {
    /// 规范化。
    Normalize,
    /// 加法。
    Add,
    /// 乘法。
    Mul,
    /// Gröbner 基。
    Groebner,
    /// 消元理想。
    Eliminate,
}

/// M-Graph / 重写缓存键（Living `11`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PolynomialCacheKey {
    /// 操作。
    pub operation: PolynomialCacheOp,
    /// 环 id。
    pub ring: RingId,
    /// 各输入 canonical hash（有序）。
    pub input_hashes: Vec<u64>,
    /// Gröbner / 消元资源指纹（非 Gröbner 操作为 0）。
    pub limits_fingerprint: u64,
}

impl PolynomialCacheKey {
    /// 稳定 hash（用于 M-Graph 边标签）。
    pub fn fingerprint(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        let mut h = DefaultHasher::new();
        self.hash(&mut h);
        h.finish()
    }
}

/// 从 [`PolynomialRequest`] 构造缓存键（输入须已 canonical 或将被 canonical 化后 miss）。
pub fn cache_key_for_request(request: &PolynomialRequest, rings: &RingTable) -> Result<PolynomialCacheKey> {
    match request {
        PolynomialRequest::Normalize { polynomial } => single_input_key(PolynomialCacheOp::Normalize, polynomial, rings, 0),
        PolynomialRequest::Add { lhs, rhs } => two_input_key(PolynomialCacheOp::Add, lhs, rhs, rings, 0),
        PolynomialRequest::Mul { lhs, rhs } => two_input_key(PolynomialCacheOp::Mul, lhs, rhs, rings, 0),
        PolynomialRequest::Groebner { generators, limits } => {
            many_input_key(PolynomialCacheOp::Groebner, generators, rings, limits_fingerprint(limits))
        }
        PolynomialRequest::Eliminate { generators, limits } => {
            many_input_key(PolynomialCacheOp::Eliminate, generators, rings, limits_fingerprint(limits))
        }
        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_unsupported")),
    }
}

fn single_input_key(op: PolynomialCacheOp, poly: &Polynomial, rings: &RingTable, limits_fp: u64) -> Result<PolynomialCacheKey> {
    Ok(PolynomialCacheKey {
        operation: op,
        ring: poly.ring,
        input_hashes: vec![polynomial_canonical_hash(poly, rings)?],
        limits_fingerprint: limits_fp,
    })
}

fn two_input_key(
    op: PolynomialCacheOp,
    lhs: &Polynomial,
    rhs: &Polynomial,
    rings: &RingTable,
    limits_fp: u64,
) -> Result<PolynomialCacheKey> {
    if lhs.ring != rhs.ring {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_ring_mismatch"));
    }
    Ok(PolynomialCacheKey {
        operation: op,
        ring: lhs.ring,
        input_hashes: vec![polynomial_canonical_hash(lhs, rings)?, polynomial_canonical_hash(rhs, rings)?],
        limits_fingerprint: limits_fp,
    })
}

fn many_input_key(
    op: PolynomialCacheOp,
    generators: &[Polynomial],
    rings: &RingTable,
    limits_fp: u64,
) -> Result<PolynomialCacheKey> {
    if generators.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_empty_generators"));
    }
    let ring = generators[0].ring;
    let mut input_hashes = Vec::with_capacity(generators.len());
    for g in generators {
        if g.ring != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "cache_key_ring_mismatch"));
        }
        input_hashes.push(polynomial_canonical_hash(g, rings)?);
    }
    input_hashes.sort_unstable();
    Ok(PolynomialCacheKey { operation: op, ring, input_hashes, limits_fingerprint: limits_fp })
}

fn limits_fingerprint(limits: &GroebnerLimits) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    limits.max_s_pairs.hash(&mut h);
    limits.max_basis_size.hash(&mut h);
    h.finish()
}
