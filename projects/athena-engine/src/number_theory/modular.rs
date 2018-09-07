//! 模逆与模幂。

use num_bigint::BigInt;
use num_traits::{One, Signed, Zero};

use athena_types::{Diagnostic, DiagnosticCode, ModularValue, Modulus};

use super::gcd::extended_gcd;

/// `a⁻¹ (mod m)`；不互素 → `ATHENA_MODULAR_INVERSE_MISSING`。
pub fn mod_inverse(a: &BigInt, modulus: &Modulus) -> Result<ModularValue, Diagnostic> {
    let m = modulus.value();
    let aa = modulus.reduce(a);
    if aa.is_zero() {
        return Err(Diagnostic::error(
            DiagnosticCode::ModularInverseMissing,
            "0 无模逆",
        ));
    }
    let eg = extended_gcd(&aa, m);
    if eg.g != BigInt::one() {
        return Err(Diagnostic::error(
            DiagnosticCode::ModularInverseMissing,
            format!("gcd({}, {}) = {} ≠ 1", aa, m, eg.g),
        ));
    }
    Ok(ModularValue::new(eg.s, modulus.clone()))
}

/// `base^exp mod m`；负指数要求先有模逆。
pub fn mod_pow(base: &BigInt, exp: &BigInt, modulus: &Modulus) -> Result<ModularValue, Diagnostic> {
    if exp.is_negative() {
        let inv = mod_inverse(base, modulus)?;
        let pos = -exp;
        let r = inv.residue().modpow(&pos, modulus.value());
        return Ok(ModularValue::new(r, modulus.clone()));
    }
    let b = modulus.reduce(base);
    let r = b.modpow(exp, modulus.value());
    Ok(ModularValue::new(r, modulus.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_and_pow() {
        let m = Modulus::new(7).unwrap();
        let inv = mod_inverse(&3.into(), &m).unwrap();
        assert_eq!(inv.residue(), &BigInt::from(5));
        let p = mod_pow(&3.into(), &4.into(), &m).unwrap();
        assert_eq!(p.residue(), &BigInt::from(4)); // 81 ≡ 4 (mod 7)
    }
}
