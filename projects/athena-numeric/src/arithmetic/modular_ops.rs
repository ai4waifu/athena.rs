//! Montgomery / Barrett 模运算内核与 batch 逆元。

use athena_types::{Diagnostic, DiagnosticCode, ModulusId, Result};

use crate::{
    integer::Integer,
    kernel::limb as limb_kernel,
    modular::ModularValue,
    modulus_context::{ModulusContext, ModulusTable},
    natural::Natural,
};

fn power_of_two_natural(bits: u32) -> Natural {
    if bits == 0 {
        return Natural::one();
    }
    let limb_idx = (bits / 64) as usize;
    let bit_in_limb = bits % 64;
    let mut limbs = vec![0u64; limb_idx + 1];
    limbs[limb_idx] = 1u64 << bit_in_limb;
    Natural::from_limbs(limbs)
}

fn barrett_mod_natural(a: &Natural, b: &Natural, modulus: &Natural, mu: &Natural, k: u32) -> Natural {
    let t = a.mul(b);
    let two_2k = power_of_two_natural(2 * k);
    let q = t.mul(mu).div_rem(&two_2k).0;
    let mut r = t.sub(&q.mul(modulus));
    while r >= *modulus {
        r = r.sub(modulus);
    }
    r
}

impl ModulusContext {
    /// `a·b mod m`（优先 Montgomery，否则 Barrett，否则朴素约化）。
    pub fn mod_mul(&self, a: &Integer, b: &Integer) -> Integer {
        let m = self.modulus.value();
        let aa = self.modulus.reduce(a);
        let bb = self.modulus.reduce(b);
        let mag = m.magnitude();
        let aa_mag = aa.magnitude();
        let bb_mag = bb.magnitude();
        if let Some(mp) = &self.montgomery {
            let prod =
                limb_kernel::mul_mod_montgomery_precomputed(aa_mag.as_limbs(), bb_mag.as_limbs(), mag.as_limbs(), mp.n_prime);
            return Integer::from_positive_natural(Natural::from_limbs(prod));
        }
        if let Some(bp) = &self.barrett {
            let r = barrett_mod_natural(&aa_mag, &bb_mag, &mag, &bp.mu, bp.k);
            return Integer::from_positive_natural(r);
        }
        aa.mul(&bb).rem_euclid(m).expect("modulus")
    }

    /// 模幂（优先 Montgomery 预计算常量）。
    pub fn mod_pow(&self, base: &Integer, exp: &Integer) -> Integer {
        if exp.is_negative() {
            return Integer::zero();
        }
        let mag = self.modulus.value().magnitude();
        let base_mag = base.abs().magnitude();
        let exp_mag = exp.abs().magnitude();
        if let Some(mp) = &self.montgomery {
            let out = limb_kernel::mod_pow_montgomery_precomputed(
                base_mag.as_limbs(),
                exp_mag.as_limbs(),
                mag.as_limbs(),
                mp.n_prime,
                mp.r2.as_limbs(),
            );
            return Integer::from_positive_natural(Natural::from_limbs(out));
        }
        Integer::from_positive_natural(base_mag.mod_pow(&exp_mag, &mag))
    }
}

/// 批量模逆（乘积树；要求各剩余在模 `m` 下可逆）。
pub fn batch_mod_inverse(table: &ModulusTable, modulus_id: ModulusId, residues: &[Integer]) -> Result<Vec<ModularValue>> {
    let ctx = table
        .get(modulus_id)
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::DomainMismatch).detail("reason", "unknown ModulusId"))?;
    let m = ctx.modulus.value();
    if residues.is_empty() {
        return Ok(Vec::new());
    }

    let mut prefix: Vec<Integer> = Vec::with_capacity(residues.len());
    let mut acc = ctx.modulus.reduce(&residues[0]);
    if acc.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("residue", "0"));
    }
    prefix.push(acc.clone());
    for r in residues.iter().skip(1) {
        acc = ctx.mod_mul(&acc, r);
        if acc.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("residue", "0"));
        }
        prefix.push(acc.clone());
    }

    let mut inv_acc = extended_gcd_inverse(&acc, m)?;
    let mut out: Vec<ModularValue> = Vec::with_capacity(residues.len());
    for i in (0..residues.len()).rev() {
        let ri = ctx.modulus.reduce(&residues[i]);
        let inv_i = if i == 0 { inv_acc.clone() } else { ctx.mod_mul(&inv_acc, &prefix[i - 1]) };
        out.push(ModularValue::new_interned(inv_i, modulus_id));
        if i > 0 {
            inv_acc = ctx.mod_mul(&inv_acc, &ri);
        }
    }
    out.reverse();
    Ok(out)
}

fn extended_gcd_inverse(a: &Integer, modulus: &Integer) -> Result<Integer> {
    let mut old_r = a.abs();
    let mut r = modulus.abs();
    let mut old_s = Integer::one();
    let mut s = Integer::zero();
    while !r.is_zero() {
        let q = old_r.div(&r).expect("nonzero remainder");
        let next_r = old_r.sub(&q.mul(&r));
        old_r = r;
        r = next_r;
        let next_s = old_s.sub(&q.mul(&s));
        old_s = s;
        s = next_s;
    }
    if !old_r.is_one() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).arg("gcd", old_r.to_decimal_string()));
    }
    Ok(old_s.rem_euclid(modulus).expect("modulus"))
}
