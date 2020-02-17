//! M-Graph 多项式缓存键（operation · 环指纹 · 输入指纹）。

use std::hash::{Hash, Hasher};

use athena_types::{Diagnostic, DiagnosticCode, Result, RingId};

use super::{
    fingerprint::{PolynomialFingerprint, RingFingerprint},
    groebner::GroebnerLimits,
    hash::canonical_hash as polynomial_canonical_hash,
    object::Polynomial,
    object_ref::{PolynomialObjectStore, PolynomialRef},
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
    /// Gröbner 基（F4）。
    GroebnerF4,
    /// 消元理想。
    Eliminate,
    /// 从 frontier 恢复 Gröbner。
    ResumeGroebner,
    /// 从 frontier 恢复 F4 Gröbner。
    ResumeGroebnerF4,
    /// ℤ/ℚ → 𝔽_p 模同态。
    ModularImage,
    /// 𝔽_p → ℤ/ℚ Wang 有理重构。
    ReconstructModular,
    /// 多素数 CRT + Wang 重构。
    CrtCombineModular,
}

impl PolynomialCacheOp {
    /// 稳定 wire / witness 标签。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Normalize => "normalize",
            Self::Add => "add",
            Self::Mul => "mul",
            Self::Groebner => "groebner",
            Self::GroebnerF4 => "groebner_f4",
            Self::Eliminate => "eliminate",
            Self::ResumeGroebner => "resume_groebner",
            Self::ResumeGroebnerF4 => "resume_groebner_f4",
            Self::ModularImage => "modular_image",
            Self::ReconstructModular => "reconstruct_modular",
            Self::CrtCombineModular => "crt_combine_modular",
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

/// 从 [`PolynomialRequest`] 构造缓存键（经 DomainObject 仓解析）。
pub fn cache_key_for_request(request: &PolynomialRequest, rings: &RingTable, store: &PolynomialObjectStore) -> Result<PolynomialCacheKey> {
    match request {
        PolynomialRequest::Normalize { polynomial } => {
            let poly = store.resolve_owning(*polynomial)?;
            single_input_key(PolynomialCacheOp::Normalize, &poly, rings, 0)
        }
        PolynomialRequest::Add { lhs, rhs } => {
            let lhs = store.resolve_owning(*lhs)?;
            let rhs = store.resolve_owning(*rhs)?;
            two_input_key(PolynomialCacheOp::Add, &lhs, &rhs, rings, 0)
        }
        PolynomialRequest::Mul { lhs, rhs } => {
            let lhs = store.resolve_owning(*lhs)?;
            let rhs = store.resolve_owning(*rhs)?;
            two_input_key(PolynomialCacheOp::Mul, &lhs, &rhs, rings, 0)
        }
        PolynomialRequest::Groebner { generators, limits } => {
            let generators = resolve_polys(store, generators)?;
            many_input_key(PolynomialCacheOp::Groebner, &generators, rings, limits_fingerprint(limits))
        }
        PolynomialRequest::GroebnerF4 { generators, limits } => {
            let generators = resolve_polys(store, generators)?;
            many_input_key(PolynomialCacheOp::GroebnerF4, &generators, rings, limits_fingerprint(limits))
        }
        PolynomialRequest::Eliminate { generators, limits } => {
            let generators = resolve_polys(store, generators)?;
            many_input_key(PolynomialCacheOp::Eliminate, &generators, rings, limits_fingerprint(limits))
        }
        PolynomialRequest::ResumeGroebner { candidates, pending_pairs, pending_insertion, input_generators, prior_s_pair_steps, limits } => {
            let candidates = resolve_polys(store, candidates)?;
            let insertion = match pending_insertion {
                Some(r) => Some(store.resolve_owning(*r)?),
                None => None,
            };
            resume_groebner_key(
                PolynomialCacheOp::ResumeGroebner,
                &candidates,
                insertion.as_ref(),
                pending_pairs,
                *input_generators,
                *prior_s_pair_steps,
                limits,
                rings,
            )
        }
        PolynomialRequest::ResumeGroebnerF4 {
            candidates,
            pending_pairs,
            pending_insertion,
            input_generators,
            prior_s_pair_steps,
            candidate_sugars,
            pending_insertion_sugar,
            limits,
        } => {
            let candidates = resolve_polys(store, candidates)?;
            let insertion = match pending_insertion {
                Some(r) => Some(store.resolve_owning(*r)?),
                None => None,
            };
            resume_groebner_key_f4(
                &candidates,
                insertion.as_ref(),
                pending_pairs,
                *input_generators,
                *prior_s_pair_steps,
                candidate_sugars.as_deref(),
                *pending_insertion_sugar,
                limits,
                rings,
            )
        }
        PolynomialRequest::ModularImage { polynomial, image_ring } => {
            let poly = store.resolve_owning(*polynomial)?;
            modular_image_key(&poly, *image_ring, rings)
        }
        PolynomialRequest::ReconstructModular { image, target_ring } => {
            let poly = store.resolve_owning(*image)?;
            reconstruct_modular_key(&poly, *target_ring, rings)
        }
        PolynomialRequest::CrtCombineModular { images, integer_ring, target_ring } => {
            let polys = resolve_polys(store, images)?;
            crt_combine_modular_key(&polys, *integer_ring, *target_ring, rings)
        }
        _ => Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_unsupported")),
    }
}

fn resolve_polys(store: &PolynomialObjectStore, refs: &[PolynomialRef]) -> Result<Vec<Polynomial>> {
    refs.iter().map(|r| store.resolve_owning(*r)).collect()
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

/// Resume keys must preserve candidate order (pair indices) and pending work.
fn resume_groebner_key(
    operation: PolynomialCacheOp,
    candidates: &[Polynomial],
    insertion: Option<&Polynomial>,
    pending_pairs: &[(usize, usize)],
    input_generators: usize,
    prior_s_pair_steps: u32,
    limits: &GroebnerLimits,
    rings: &RingTable,
) -> Result<PolynomialCacheKey> {
    if candidates.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_empty_resume_candidates"));
    }
    let ring = candidates[0].ring();
    let ring_fingerprint = ring_fingerprint_for(&candidates[0], rings)?;
    let mut input_fingerprints = Vec::with_capacity(candidates.len() + usize::from(insertion.is_some()));
    let mut input_hashes = Vec::with_capacity(candidates.len() + usize::from(insertion.is_some()));
    for g in candidates {
        if g.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "cache_key_ring_mismatch"));
        }
        input_fingerprints.push(poly_fingerprint(g, rings)?);
        input_hashes.push(polynomial_canonical_hash(g, rings)?);
    }
    if let Some(ins) = insertion {
        if ins.ring() != ring {
            return Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .detail("domain", "polynomial")
                .detail("operation", "cache_key_ring_mismatch"));
        }
        input_fingerprints.push(poly_fingerprint(ins, rings)?);
        input_hashes.push(polynomial_canonical_hash(ins, rings)?);
    }
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    limits.max_s_pairs.hash(&mut h);
    limits.max_basis_size.hash(&mut h);
    input_generators.hash(&mut h);
    prior_s_pair_steps.hash(&mut h);
    pending_pairs.len().hash(&mut h);
    for &(i, j) in pending_pairs {
        i.hash(&mut h);
        j.hash(&mut h);
    }
    insertion.is_some().hash(&mut h);
    Ok(PolynomialCacheKey { operation, ring, ring_fingerprint, input_fingerprints, input_hashes, limits_fingerprint: h.finish() })
}

fn resume_groebner_key_f4(
    candidates: &[Polynomial],
    insertion: Option<&Polynomial>,
    pending_pairs: &[(usize, usize)],
    input_generators: usize,
    prior_s_pair_steps: u32,
    candidate_sugars: Option<&[u32]>,
    pending_insertion_sugar: Option<u32>,
    limits: &GroebnerLimits,
    rings: &RingTable,
) -> Result<PolynomialCacheKey> {
    let mut key = resume_groebner_key(
        PolynomialCacheOp::ResumeGroebnerF4,
        candidates,
        insertion,
        pending_pairs,
        input_generators,
        prior_s_pair_steps,
        limits,
        rings,
    )?;
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    key.limits_fingerprint.hash(&mut h);
    match candidate_sugars {
        Some(s) => {
            s.len().hash(&mut h);
            for sugar in s {
                sugar.hash(&mut h);
            }
        }
        None => 0usize.hash(&mut h),
    }
    pending_insertion_sugar.hash(&mut h);
    key.limits_fingerprint = h.finish();
    Ok(key)
}

fn modular_image_key(poly: &Polynomial, image_ring: RingId, rings: &RingTable) -> Result<PolynomialCacheKey> {
    let image_fp = rings.ring_fingerprint(image_ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation).detail("domain", "polynomial").detail("operation", "cache_key_unknown_image_ring")
    })?;
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    image_fp.hash(&mut h);
    single_input_key(PolynomialCacheOp::ModularImage, poly, rings, h.finish())
}

fn reconstruct_modular_key(poly: &Polynomial, target_ring: RingId, rings: &RingTable) -> Result<PolynomialCacheKey> {
    let target_fp = rings.ring_fingerprint(target_ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_unknown_target_ring")
    })?;
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    target_fp.hash(&mut h);
    single_input_key(PolynomialCacheOp::ReconstructModular, poly, rings, h.finish())
}

fn crt_combine_modular_key(images: &[Polynomial], integer_ring: RingId, target_ring: RingId, rings: &RingTable) -> Result<PolynomialCacheKey> {
    if images.len() < 2 {
        return Err(Diagnostic::new(DiagnosticCode::DomainError)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_crt_combine_too_few_images"));
    }
    let integer_fp = rings.ring_fingerprint(integer_ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_unknown_integer_ring")
    })?;
    let target_fp = rings.ring_fingerprint(target_ring).ok_or_else(|| {
        Diagnostic::new(DiagnosticCode::UnsupportedOperation)
            .detail("domain", "polynomial")
            .detail("operation", "cache_key_unknown_target_ring")
    })?;
    use std::collections::hash_map::DefaultHasher;
    let mut h = DefaultHasher::new();
    integer_fp.hash(&mut h);
    target_fp.hash(&mut h);
    let mut input_fingerprints = Vec::with_capacity(images.len());
    let mut input_hashes = Vec::with_capacity(images.len());
    for g in images {
        let ring_fp = ring_fingerprint_for(g, rings)?;
        ring_fp.hash(&mut h);
        input_fingerprints.push(poly_fingerprint(g, rings)?);
        input_hashes.push(polynomial_canonical_hash(g, rings)?);
    }
    // Order-independent: CRT inputs commute.
    input_fingerprints.sort_unstable();
    input_hashes.sort_unstable();
    Ok(PolynomialCacheKey {
        operation: PolynomialCacheOp::CrtCombineModular,
        ring: integer_ring,
        ring_fingerprint: integer_fp,
        input_fingerprints,
        input_hashes,
        limits_fingerprint: h.finish(),
    })
}
