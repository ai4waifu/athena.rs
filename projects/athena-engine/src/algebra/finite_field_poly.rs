//! 𝔽_{p^n} 多项式基 presentation 数据与算术（Living `18` Phase 5）。

use athena_numeric::{Integer, Modulus};
use athena_types::{Diagnostic, DiagnosticCode, ExtensionId, FieldId, Result};

/// 不可变 𝔽_{p^n} 多项式基规格（由 [`super::table::FieldTable`] 持有）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiniteFieldPolySpec {
    /// 扩张 id。
    pub extension: ExtensionId,
    /// 基素域 𝔽_p。
    pub base: FieldId,
    /// 特征 p。
    pub characteristic: Integer,
    /// 扩张次数 n。
    pub degree: u32,
    /// 首一不可约多项式，升幂系数 `[c0, …, c_{n-1}, 1]`。
    pub modulus: Vec<Integer>,
}

/// 将模多项式系数规范到 `[0, p)`。
pub fn canonicalize_modulus(coeffs: Vec<Integer>, p: &Modulus) -> Result<Vec<Integer>> {
    Ok(coeffs.into_iter().map(|c| p.reduce(&c)).collect())
}

/// 校验首一模多项式形状并返回次数 n。
pub fn validate_modulus_shape(coeffs: &[Integer], p: &Modulus) -> Result<u32> {
    if coeffs.len() < 3 {
        return Err(Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
            .detail("domain", "field")
            .detail("operation", "modulus_degree_too_small"));
    }
    if !coeffs.last().map(|c| c.is_one()).unwrap_or(false) {
        return Err(Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
            .detail("domain", "field")
            .detail("operation", "modulus_not_monic"));
    }
    for c in coeffs {
        let r = p.reduce(c);
        if r != *c {
            return Err(Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
                .detail("domain", "field")
                .detail("operation", "modulus_coeff_not_reduced"));
        }
    }
    let n = u32::try_from(coeffs.len() - 1).map_err(|_| {
        Diagnostic::new(DiagnosticCode::FieldExtensionInvalid)
            .detail("domain", "field")
            .detail("operation", "modulus_degree_overflow")
    })?;
    Ok(n)
}

/// 不可约性：无 𝔽_p 根，且无次数 ≤ n/2 的首一因子。
pub fn is_irreducible_monic(coeffs: &[Integer], p: &Modulus) -> Result<bool> {
    let n = validate_modulus_shape(coeffs, p)? as usize;
    if n < 2 {
        return Ok(false);
    }
    if has_root_in_fp(coeffs, p) {
        return Ok(false);
    }
    if n == 2 || n == 3 {
        return Ok(true);
    }
    for d in 2..=(n / 2) {
        if has_monic_factor_of_degree(coeffs, d, p)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 规范元素坐标（长度 n，系数 ∈ `[0, p)`）。
pub fn canonical_coords(mut coords: Vec<Integer>, degree: u32, p: &Modulus) -> Result<Vec<Integer>> {
    let n = usize::try_from(degree).map_err(|_| {
        Diagnostic::new(DiagnosticCode::FieldElementInvalid)
            .detail("domain", "field")
            .detail("operation", "extension_degree_overflow")
    })?;
    if coords.len() > n {
        return Err(Diagnostic::new(DiagnosticCode::FieldElementInvalid)
            .detail("domain", "field")
            .detail("operation", "extension_coord_length"));
    }
    coords.resize(n, Integer::zero());
    for c in &mut coords {
        *c = p.reduce(c);
    }
    Ok(coords)
}

/// 多项式基坐标加法。
pub fn add_coords(a: &[Integer], b: &[Integer], p: &Modulus) -> Vec<Integer> {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| p.reduce(&x.add(y)))
        .collect()
}

/// 多项式基坐标乘法（mod 不可约多项式）。
pub fn mul_coords(a: &[Integer], b: &[Integer], spec: &FiniteFieldPolySpec, p: &Modulus) -> Vec<Integer> {
    let product = poly_mul(a, b, p);
    poly_mod(&product, &spec.modulus, p)
}

/// 多项式基坐标乘法逆元（扩展 gcd）。
pub fn inv_coords(a: &[Integer], spec: &FiniteFieldPolySpec, p: &Modulus) -> Result<Vec<Integer>> {
    if a.iter().all(|c| c.is_zero()) {
        return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "field"));
    }
    let (_, s, _) = poly_extended_gcd(a, &spec.modulus, p)?;
    let inv = poly_mod(&s, &spec.modulus, p);
    Ok(inv)
}

fn has_monic_factor_of_degree(f: &[Integer], degree: usize, p: &Modulus) -> Result<bool> {
    let prime = p.value();
    let mut coeffs = vec![Integer::zero(); degree + 1];
    coeffs[degree] = Integer::one();
    loop {
        if divides_monic(f, &coeffs, p)? {
            return Ok(true);
        }
        if !increment_monic_coeffs(&mut coeffs, degree, prime)? {
            break;
        }
    }
    Ok(false)
}

fn divides_monic(f: &[Integer], g: &[Integer], p: &Modulus) -> Result<bool> {
    let (_, rem) = poly_div_rem(f, g, p)?;
    Ok(rem.iter().all(|c| c.is_zero()))
}

fn increment_monic_coeffs(coeffs: &mut [Integer], degree: usize, prime: &Integer) -> Result<bool> {
    for i in 0..degree {
        coeffs[i] = coeffs[i].add(&Integer::one());
        if coeffs[i].cmp(prime) == std::cmp::Ordering::Less {
            return Ok(true);
        }
        coeffs[i] = Integer::zero();
    }
    Ok(false)
}

fn has_root_in_fp(coeffs: &[Integer], p: &Modulus) -> bool {
    let prime = p.value();
    let mut x = Integer::zero();
    loop {
        if poly_eval(coeffs, &x, p).is_zero() {
            return true;
        }
        if x.add(&Integer::one()).cmp(prime) != std::cmp::Ordering::Less {
            break;
        }
        x = x.add(&Integer::one());
    }
    false
}

fn poly_eval(coeffs: &[Integer], x: &Integer, p: &Modulus) -> Integer {
    let mut acc = Integer::zero();
    let mut pow = Integer::one();
    for c in coeffs {
        acc = p.reduce(&acc.add(&p.reduce(&c.mul(&pow))));
        pow = p.reduce(&pow.mul(x));
    }
    acc
}

fn poly_mul(a: &[Integer], b: &[Integer], p: &Modulus) -> Vec<Integer> {
    if a.is_empty() || b.is_empty() {
        return Vec::new();
    }
    let mut out = vec![Integer::zero(); a.len() + b.len() - 1];
    for (i, ai) in a.iter().enumerate() {
        for (j, bj) in b.iter().enumerate() {
            out[i + j] = p.reduce(&out[i + j].add(&p.reduce(&ai.mul(bj))));
        }
    }
    trim_poly(&mut out);
    out
}

fn poly_mod(a: &[Integer], modulus: &[Integer], p: &Modulus) -> Vec<Integer> {
    let n = modulus.len() - 1;
    let mut r = a.to_vec();
    trim_poly(&mut r);
    while r.len() > n {
        let deg = r.len() - 1;
        let lead = r[deg].clone();
        if !lead.is_zero() {
            let shift = deg - n;
            if r.len() < shift + modulus.len() {
                r.resize(shift + modulus.len(), Integer::zero());
            }
            for (i, mi) in modulus.iter().enumerate() {
                r[shift + i] = p.reduce(&r[shift + i].sub(&p.reduce(&lead.mul(mi))));
            }
        }
        r.pop();
        trim_poly(&mut r);
    }
    r.resize(n, Integer::zero());
    for c in &mut r {
        *c = p.reduce(c);
    }
    r
}

fn poly_sub(a: &[Integer], b: &[Integer], p: &Modulus) -> Vec<Integer> {
    let len = a.len().max(b.len());
    let mut out = vec![Integer::zero(); len];
    for (i, ai) in a.iter().enumerate() {
        out[i] = ai.clone();
    }
    for (i, bi) in b.iter().enumerate() {
        out[i] = p.reduce(&out[i].sub(bi));
    }
    trim_poly(&mut out);
    out
}

fn poly_extended_gcd(a: &[Integer], b: &[Integer], p: &Modulus) -> Result<(Vec<Integer>, Vec<Integer>, Vec<Integer>)> {
    let mut old_r = a.to_vec();
    let mut r = b.to_vec();
    trim_poly(&mut old_r);
    trim_poly(&mut r);
    let mut old_s = vec![Integer::one()];
    let mut s = Vec::new();
    let mut old_t = Vec::new();
    let mut t = vec![Integer::one()];
    while !r.is_empty() && !(r.len() == 1 && r[0].is_zero()) {
        let (q, rem) = poly_div_rem(&old_r, &r, p)?;
        old_r = r;
        r = rem;
        let q_s = poly_mul(&q, &s, p);
        let new_s = poly_sub(&old_s, &q_s, p);
        old_s = s;
        s = new_s;
        let q_t = poly_mul(&q, &t, p);
        let new_t = poly_sub(&old_t, &q_t, p);
        old_t = t;
        t = new_t;
    }
    normalize_monic(&mut old_r, p);
    trim_poly(&mut old_r);
    if !(old_r.len() == 1 && old_r[0].is_one()) {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("domain", "field"));
    }
    Ok((old_r, old_s, old_t))
}

fn poly_div_rem(a: &[Integer], b: &[Integer], p: &Modulus) -> Result<(Vec<Integer>, Vec<Integer>)> {
    if b.is_empty() || b.iter().all(|c| c.is_zero()) {
        return Ok((Vec::new(), a.to_vec()));
    }
    let deg_b = b.len() - 1;
    let lc = b.last().cloned().unwrap_or_else(Integer::one);
    let lc_inv = crate::number_theory::mod_inverse(&lc, p)?.residue().clone();
    let mut quotient = Vec::new();
    let mut remainder = a.to_vec();
    trim_poly(&mut remainder);
    loop {
        trim_poly(&mut remainder);
        if remainder.iter().all(|c| c.is_zero()) {
            break;
        }
        let deg_r = remainder.len() - 1;
        if deg_r < deg_b {
            break;
        }
        let lead = remainder[deg_r].clone();
        if lead.is_zero() {
            remainder.pop();
            continue;
        }
        let shift = deg_r - deg_b;
        let scale = p.reduce(&lead.mul(&lc_inv));
        if quotient.len() < shift + 1 {
            quotient.resize(shift + 1, Integer::zero());
        }
        quotient[shift] = p.reduce(&quotient[shift].add(&scale));
        if remainder.len() < shift + b.len() {
            remainder.resize(shift + b.len(), Integer::zero());
        }
        for (i, bi) in b.iter().enumerate() {
            remainder[shift + i] = p.reduce(&remainder[shift + i].sub(&p.reduce(&scale.mul(bi))));
        }
    }
    trim_poly(&mut quotient);
    Ok((quotient, remainder))
}

fn trim_poly(v: &mut Vec<Integer>) {
    while v.len() > 1 && v.last().is_some_and(|c| c.is_zero()) {
        v.pop();
    }
    if v.is_empty() {
        v.push(Integer::zero());
    }
}

fn normalize_monic(v: &mut Vec<Integer>, p: &Modulus) {
    trim_poly(v);
    if v.is_empty() || v.iter().all(|c| c.is_zero()) {
        *v = vec![Integer::zero()];
        return;
    }
    let lc = v.last().cloned().unwrap();
    if lc.is_one() {
        return;
    }
    if let Ok(inv) = crate::number_theory::mod_inverse(&lc, p) {
        let inv = inv.residue().clone();
        for c in v.iter_mut() {
            *c = p.reduce(&c.mul(&inv));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gf4_mul_square() {
        let p = Modulus::new(Integer::from_i64(2)).unwrap();
        let spec = FiniteFieldPolySpec {
            extension: ExtensionId(0),
            base: FieldId(0),
            characteristic: Integer::from_i64(2),
            degree: 2,
            modulus: vec![Integer::one(), Integer::one(), Integer::one()],
        };
        let a = vec![Integer::one(), Integer::one()];
        let sq = mul_coords(&a, &a, &spec, &p);
        assert_eq!(sq, vec![Integer::zero(), Integer::one()]);
    }

    #[test]
    fn gf4_inverse_of_x() {
        let p = Modulus::new(Integer::from_i64(2)).unwrap();
        let spec = FiniteFieldPolySpec {
            extension: ExtensionId(0),
            base: FieldId(0),
            characteristic: Integer::from_i64(2),
            degree: 2,
            modulus: vec![Integer::one(), Integer::one(), Integer::one()],
        };
        let x = vec![Integer::zero(), Integer::one()];
        let inv = inv_coords(&x, &spec, &p).unwrap();
        let prod = mul_coords(&x, &inv, &spec, &p);
        assert_eq!(prod, vec![Integer::one(), Integer::zero()]);
    }
}
