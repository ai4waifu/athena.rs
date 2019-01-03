//! 模逆、模幂与 batch 逆元（[`ModulusTable`] + [`ModulusId`] 执行路径）。

use athena_numeric::{Integer, ModularValue, Modulus, ModulusTable, batch_mod_inverse as numeric_batch_inverse};
use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::gcd::extended_gcd;

/// `a⁻¹ (mod m)`；不互素 → `ATHENA_MODULAR_INVERSE_MISSING`。
pub fn mod_inverse(a: &Integer, modulus: &Modulus) -> Result<ModularValue> {
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

/// 经 [`ModulusTable`] intern 后返回 [`ModulusId`] 绑定的 [`ModularValue`]。
pub fn mod_inverse_with_table(a: &Integer, modulus: &Modulus, table: &mut ModulusTable) -> Result<ModularValue> {
    let id = table.intern(modulus.clone());
    let aa = modulus.reduce(a);
    if aa.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("residue", "0"));
    }
    let eg = extended_gcd(&aa, modulus.value());
    if !eg.g.is_one() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).arg("gcd", eg.g.to_decimal_string()));
    }
    Ok(ModularValue::new_interned(modulus.reduce(&eg.s), id))
}

/// `base^exp mod m`；负指数要求先有模逆。
pub fn mod_pow(base: &Integer, exp: &Integer, modulus: &Modulus) -> Result<ModularValue> {
    if exp.is_negative() {
        let inv = mod_inverse(base, modulus)?;
        let pos = exp.neg();
        let r = inv.residue().mod_pow(&pos, modulus.value()).expect("mod_pow");
        return Ok(ModularValue::new(r, modulus.clone()));
    }
    let b = modulus.reduce(base);
    let r = b.mod_pow(exp, modulus.value()).expect("mod_pow");
    Ok(ModularValue::new(r, modulus.clone()))
}

/// 经 [`ModulusContext`] 内核（Montgomery/Barrett）计算模幂。
pub fn mod_pow_with_table(base: &Integer, exp: &Integer, modulus: &Modulus, table: &mut ModulusTable) -> Result<ModularValue> {
    let id = table.intern(modulus.clone());
    if exp.is_negative() {
        let inv = mod_inverse_with_table(base, modulus, table)?;
        let pos = exp.neg();
        let r = table.get(id).expect("just interned").mod_pow(inv.residue(), &pos);
        return Ok(ModularValue::new_interned(r, id));
    }
    let r = table.get(id).expect("just interned").mod_pow(base, exp);
    Ok(ModularValue::new_interned(r, id))
}

/// 批量模逆（乘积树；prime / 互素剩余）。
pub fn batch_mod_inverse(residues: &[Integer], modulus: &Modulus, table: &mut ModulusTable) -> Result<Vec<ModularValue>> {
    let id = table.intern(modulus.clone());
    numeric_batch_inverse(table, id, residues)
}
