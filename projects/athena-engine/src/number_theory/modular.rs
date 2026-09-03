//! 模逆、模幂与 batch 逆元（[`ModulusTable`] + [`ModulusId`] 执行路径）。

use athena_numeric::{Integer, ModularValue, Modulus, ModulusTable, batch_mod_inverse as numeric_batch_inverse};
use athena_types::{Diagnostic, DiagnosticCode, Result};

use super::gcd::extended_gcd;
use crate::numeric_clone::clone_modulus;

/// `a⁻¹ (mod m)`；不互素 → `ATHENA_MODULAR_INVERSE_MISSING`。
pub fn mod_inverse(a: &Integer, modulus: &Modulus) -> Result<ModularValue> {
    let m = modulus.value();
    let aa = modulus.reduce(a);
    if aa.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("residue", "0"));
    }
    let eg = extended_gcd(&aa, &m);
    if !eg.g.is_one() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing)
            .arg("gcd", eg.g.to_decimal_string())
            .detail("residue", aa.to_decimal_string())
            .detail("modulus", m.to_decimal_string()));
    }
    Ok(ModularValue::new(eg.s, clone_modulus(&modulus)))
}

/// 经 [`ModulusTable`] intern 后返回 [`ModulusId`] 绑定的 [`ModularValue`]。
pub fn mod_inverse_with_table(a: &Integer, modulus: &Modulus, table: &mut ModulusTable) -> Result<ModularValue> {
    let id = table.intern(clone_modulus(&modulus));
    let aa = modulus.reduce(a);
    if aa.is_zero() {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing).detail("residue", "0"));
    }
    let eg = extended_gcd(&aa, &modulus.value());
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
        let mv = modulus.value();
        let r = inv.residue().mod_pow(&pos, &mv).expect("mod_pow");
        return Ok(ModularValue::new(r, clone_modulus(&modulus)));
    }
    let b = modulus.reduce(base);
    let mv = modulus.value();
    let r = b.mod_pow(exp, &mv).expect("mod_pow");
    Ok(ModularValue::new(r, clone_modulus(&modulus)))
}

/// 经 [`ModulusContext`] 内核（Montgomery/Barrett）计算模幂。
pub fn mod_pow_with_table(base: &Integer, exp: &Integer, modulus: &Modulus, table: &mut ModulusTable) -> Result<ModularValue> {
    let id = table.intern(clone_modulus(&modulus));
    if exp.is_negative() {
        let inv = mod_inverse_with_table(base, modulus, table)?;
        let pos = exp.neg();
        let base_res = inv.residue();
        let r = table.get(id).expect("just interned").mod_pow(&base_res, &pos);
        return Ok(ModularValue::new_interned(r, id));
    }
    let r = table.get(id).expect("just interned").mod_pow(base, exp);
    Ok(ModularValue::new_interned(r, id))
}

/// 批量模逆（乘积树；prime / 互素剩余）。
pub fn batch_mod_inverse(residues: &[Integer], modulus: &Modulus, table: &mut ModulusTable) -> Result<Vec<ModularValue>> {
    let id = table.intern(clone_modulus(&modulus));
    numeric_batch_inverse(table, id, residues)
}
