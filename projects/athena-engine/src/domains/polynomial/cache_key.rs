//! M-Graph 多项式缓存键（operation · 环指纹 · 输入指纹）。

use std::hash::{Hash, Hasher};

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    object::Polynomial,
    fingerprint::{PolynomialFingerprint, RingFingerprint},
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

impl PolynomialCacheOp {
    /// 稳定 wire / witness 标签。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normalize => "normalize",
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Groebner => "groebner",
            Self::Eliminate => "eliminate",
        }
    }
}

/// M-Graph / 重写缓存键。
#[derive(Debug, Clone)]
pub struct PolynomialCacheKey {
    /// 操作。
    pub operation: PolynomialCacheOp,
    /// Session 内环句柄（执行路径；不参与跨 Session 相等性）。
    pub ring: RingId,
    /// 稳定环身份。
    pub ring_fingerprint: RingFingerprint,
    /// 各输入 stable 指纹（有序）。
    pub input_fingerprints: Vec<PolynomialFingerprint>,
    /// canonical hash 载荷（witness 摘要；与 `input_fingerprints` 一致）。
    pub input_hashes: Vec<u64>,
    /// Gröbner / 消元资源指纹（非 Gröbner 操作为 0）。
    pub limits_fingerprint: u64,
}

impl PartialEq for PolynomialCacheKey {
    fn eq(&self, other: &Self) -> bool {
        self.operation == other.operation
            && self.ring_fingerprint == other.ring_fingerprint
            && self.input_fingerprints == other.input_fingerprints
            && self.limits_fingerprint == other.limits_fingerprint
    }
}

impl Eq for PolynomialCacheKey {}

impl Hash for PolynomialCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.operation.hash(state);
        self.ring_fingerprint.hash(state);
        self.input_fingerprints.hash(state);
        self.limits_fingerprint.hash(state);
    }
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

fn ring_fingerprint_for(poly: &Polynomial, rings: &RingTable) -> Result<RingFingerprint> {
    rings.ring_fingerprint(poly.ring()).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "polynomial").detail("operation", "cache_key_unknown_ring")
    })
}

fn poly_fingerprint(poly: &Polynomial, rings: &RingTable) -> Result<PolynomialFingerprint> {
    PolynomialFingerprint::from_polynomial(poly, rings)
}

fn single_input_key(op: PolynomialCacheOp, poly: &Polynomial, rings: &RingTable, limits_fp: u64) -> Result<PolynomialCacheKey> {
    let fp = poly_fingerprint(poly, rings)?;
    Ok(PolynomialCacheKey {
        operation: op,
        ring: poly.ring(),
        ring_fingerprint: ring_fingerprint_for(poly, rings)?,
        input_fingerprints: vec![fp],
        input_hashes: vec![polynomial_canonical_hash(poly, rings)?],
        limits_fingerprint: limits_fp,
    })
}

fn two_input_key(op: PolynomialCacheOp, lhs: &Polynomial, rhs: &Polynomial, rings: &RingTable, limits_fp: u64) -> Result<PolynomialCacheKey> {
    if lhs.ring() != rhs.ring() {
        return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_ring_mismatch"));
    }
    let lfp = poly_fingerprint(lhs, rings)?;
    let rfp = poly_fingerprint(rhs, rings)?;
    Ok(PolynomialCacheKey {
        operation: op,
        ring: lhs.ring(),
        ring_fingerprint: ring_fingerprint_for(lhs, rings)?,
        input_fingerprints: vec![lfp, rfp],
        input_hashes: vec![polynomial_canonical_hash(lhs, rings)?, polynomial_canonical_hash(rhs, rings)?],
        limits_fingerprint: limits_fp,
    })
}

fn many_input_key(op: PolynomialCacheOp, generators: &[Polynomial], rings: &RingTable, limits_fp: u64) -> Result<PolynomialCacheKey> {
    if generators.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_empty_generators"));
    }
    let ring = generators[0].ring();
    let ring_fingerprint = ring_fingerprint_for(&generators[0], rings)?;
    let mut input_fingerprints = Vec::with_capacity(generators.len());
    let mut input_hashes = Vec::with_capacity(generators.len());
    for g in generators {
        if g.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "cache_key_ring_mismatch"));
        }
        input_fingerprints.push(poly_fingerprint(g, rings)?);
        input_hashes.push(polynomial_canonical_hash(g, rings)?);
    }
    input_fingerprints.sort_unstable();
    input_hashes.sort_unstable();
    Ok(PolynomialCacheKey { operation: op, ring, ring_fingerprint, input_fingerprints, input_hashes, limits_fingerprint: limits_fp })
}

fn limits_fingerprint(limits: &GroebnerLimits) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    limits.max_s_pairs.hash(&mut h);
    limits.max_basis_size.hash(&mut h);
    h.finish()
}
