//! 模逆与模幂。

use athena_numeric::{Integer, ModularValue, Modulus};
use athena_types::{Diagnostic, DiagnosticCode};

use super::gcd::extended_gcd;

/// `a⁻¹ (mod m)`；不互素 → `ATHENA_MODULAR_INVERSE_MISSING`。
pub fn mod_inverse(a: &Integer, modulus: &Modulus) -> Result<ModularValue, Diagnostic> {
    let m = modulus.value();
    let aa = modulus.reduce(a);
    if aa.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("residue", "0"));
    }
    let eg = extended_gcd(&aa, m);
    if !eg.g.is_one() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing)
            .arg("gcd", eg.g.to_decimal_string())
            .detail("residue", aa.to_decimal_string())
            .detail("modulus", m.to_decimal_string()));
    }
    Ok(ModularValue::new(eg.s, modulus.clone()))
}

/// `base^exp mod m`；负指数要求先有模逆。
pub fn mod_pow(base: &Integer, exp: &Integer, modulus: &Modulus) -> Result<ModularValue, Diagnostic> {
    if exp.is_negative() {
        let inv = mod_inverse(base, modulus)?;
        let pos = exp.neg();
        let r = inv.residue().mod_pow(&pos, modulus.value());
        return Ok(ModularValue::new(r, modulus.clone()));
    }
    let b = modulus.reduce(base);
    let r = b.mod_pow(exp, modulus.value());
    Ok(ModularValue::new(r, modulus.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_and_pow() {
        let m = Modulus::new(7).unwrap();
        let inv = mod_inverse(&3.into(), &m).unwrap();
        assert_eq!(inv.residue(), &Integer::from_i64(5));
        let p = mod_pow(&3.into(), &4.into(), &m).unwrap();
        assert_eq!(p.residue(), &Integer::from_i64(4)); // 81 ≡ 4 (mod 7)
    }
}
