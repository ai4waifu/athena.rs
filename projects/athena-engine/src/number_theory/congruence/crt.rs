//! 广义中国剩余定理。

use athena_numeric::{Integer, ModularValue, Modulus};
use athena_types::{Diagnostic, DiagnosticCode};

use crate::numeric_clone::{clone_integer};
use super::super::{
    gcd::{extended_gcd, lcm},
    result::NumberTheoryResult,
    value::{CrtResult, NumberTheoryValue},
};

/// 两两广义 CRT：`x ≡ a (mod m)` 与 `x ≡ b (mod n)`。
pub fn chinese_remainder_pair(a: &Integer, m: &Modulus, b: &Integer, n: &Modulus) -> Result<CrtResult, Diagnostic> {
    let mv = m.value();
    let nv = n.value();
    let a_red = m.reduce(a);
    let b_red = n.reduce(b);
    let eg = extended_gcd(&mv, &nv);
    let g = eg.g;
    let diff = a_red.sub(&b_red);
    let mut diff_mod_g = diff.rem(&g).expect("rem");
    if diff_mod_g.is_negative() {
        diff_mod_g = diff_mod_g.add(&g);
    }
    if !diff_mod_g.is_zero() {
        return Ok(CrtResult::Inconsistent { left_index: 0, right_index: 1, gcd: g, residue_difference: diff });
    }

    // CRT：x = a + m·t，其中 t ≡ (b−a)/g · (m/g)⁻¹ (mod n/g)
    let m_g = mv.div(&g).expect("div");
    let n_g = nv.div(&g).expect("div");
    let eg2 = extended_gcd(&m_g, &n_g);
    if !eg2.g.is_one() {
        return Err(Diagnostic::new(DiagnosticCode::CongruenceInconsistent)
            .detail("domain", "number_theory")
            .detail("operation", "chinese_remainder_pair")
            .detail("reason", "reduced_moduli_not_coprime"));
    }
    let inv = eg2.s;
    let mut t = b_red.sub(&a_red).div(&g).expect("div").mul(&inv).rem(&n_g).expect("rem");
    if t.is_negative() {
        t = t.add(&n_g);
    }
    let x = a_red.add(&mv.mul(&t));
    let l = lcm(&mv, &nv);
    let modulus_lcm = Modulus::new(l)?;
    Ok(CrtResult::Consistent { solution: ModularValue::new(x, modulus_lcm.clone()), modulus_lcm })
}

/// 多方程广义 CRT：`residues[i] (mod moduli[i])`。长度须一致且 ≥ 1。
pub fn chinese_remainder(residues: &[Integer], moduli: &[Modulus]) -> NumberTheoryResult {
    if residues.len() != moduli.len() {
        return NumberTheoryResult::InvalidInput {
            reason: Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "number_theory")
                .detail("operation", "chinese_remainder")
                .detail("reason", "residues_moduli_length_mismatch"),
        };
    }
    if residues.is_empty() {
        return NumberTheoryResult::InvalidInput {
            reason: Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "number_theory")
                .detail("operation", "chinese_remainder")
                .detail("reason", "empty_system"),
        };
    }
    if residues.len() == 1 {
        let m = moduli[0].clone();
        return NumberTheoryResult::Exact {
            value: NumberTheoryValue::Crt(CrtResult::Consistent {
                solution: ModularValue::new(residues[0].clone(), m.clone()),
                modulus_lcm: m,
            }),
        };
    }

    let mut cur_res = residues[0].clone();
    let mut cur_mod = moduli[0].clone();
    for i in 1..residues.len() {
        match chinese_remainder_pair(&cur_res, &cur_mod, &residues[i], &moduli[i]) {
            Ok(CrtResult::Consistent { solution, modulus_lcm }) => {
                cur_res = clone_integer(&solution.residue());
                cur_mod = modulus_lcm;
            }
            Ok(CrtResult::Inconsistent { gcd, residue_difference, .. }) => {
                return NumberTheoryResult::Exact {
                    value: NumberTheoryValue::Crt(CrtResult::Inconsistent {
                        left_index: 0,
                        right_index: i,
                        gcd,
                        residue_difference,
                    }),
                };
            }
            Err(reason) => return NumberTheoryResult::Unevaluated { reason },
        }
    }

    NumberTheoryResult::Exact {
        value: NumberTheoryValue::Crt(CrtResult::Consistent {
            solution: ModularValue::new(cur_res, cur_mod.clone()),
            modulus_lcm: cur_mod,
        }),
    }
}
