//! 线性同余 `a x ≡ b (mod m)`。

use athena_numeric::{Integer, ModularValue, Modulus};
use athena_types::{Diagnostic, DiagnosticCode};

use super::super::{
    gcd::extended_gcd,
    result::NumberTheoryResult,
    value::{CongruenceSolution, NumberTheoryValue},
};
use crate::runtime::values::numeric_clone::{clone_integer, clone_modulus};

/// 求解 `a x ≡ b (mod m)`。`m` 须为已验证 [`Modulus`]。
pub fn solve_linear_congruence(a: &Integer, b: &Integer, modulus: &Modulus) -> NumberTheoryResult {
    let m = modulus.value();
    let eg = extended_gcd(a, &m);
    let g = eg.g;
    let b_mod_g = {
        let mut r = b.rem(&g).expect("rem");
        if r.is_negative() {
            r = r.add(&g);
        }
        r
    };
    if !b_mod_g.is_zero() {
        return NumberTheoryResult::Exact {
            value: NumberTheoryValue::Congruence(CongruenceSolution::NoSolution { gcd: g, residue_mod_gcd: b_mod_g }),
        };
    }

    // 解 a' x ≡ b' (mod m')，其中 a'=a/g, b'=b/g, m'=m/g。
    let a_red = a.div(&g).expect("div");
    let b_red = b.div(&g).expect("div");
    let m_red = m.div(&g).expect("div");
    let eg2 = extended_gcd(&a_red, &m_red);
    // Bézout：eg2.s · a_red ≡ 1 (mod m_red)，当 gcd = 1。
    if !eg2.g.is_one() {
        return NumberTheoryResult::Unevaluated {
            reason: Diagnostic::new(DiagnosticCode::CongruenceInconsistent)
                .detail("domain", "number_theory")
                .detail("operation", "solve_linear_congruence")
                .detail("reason", "reduced_system_not_coprime"),
        };
    }
    let mut x0 = eg2.s.mul(&b_red).rem(&m_red).expect("rem");
    if x0.is_negative() {
        x0 = x0.add(&m_red);
    }

    if g.is_one() {
        return NumberTheoryResult::Exact {
            value: NumberTheoryValue::Congruence(CongruenceSolution::UniqueClass { residue: ModularValue::new(x0, clone_modulus(&modulus)) }),
        };
    }

    let reduced = match Modulus::new(clone_integer(&m_red)) {
        Ok(m) => m,
        Err(reason) => {
            // m/g = 1 时：唯一解模 1 无意义，但数学上所有整数同余；用 UniqueClass 于原模。
            if m_red.is_one() {
                return NumberTheoryResult::Exact {
                    value: NumberTheoryValue::Congruence(CongruenceSolution::UniqueClass {
                        residue: ModularValue::new(x0, clone_modulus(&modulus)),
                    }),
                };
            }
            return NumberTheoryResult::InvalidInput { reason };
        }
    };

    NumberTheoryResult::Exact {
        value: NumberTheoryValue::Congruence(CongruenceSolution::MultipleClasses {
            base_residue: x0,
            reduced_modulus: reduced,
            ambient_modulus: clone_modulus(&modulus),
            multiplicity: g,
        }),
    }
}
