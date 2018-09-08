//! 模数合同 — 与 `ModularValue` 绑定，禁止跨模静默合并。

use num_bigint::BigInt;
use num_traits::{One, Signed};

use crate::diagnostic::{Diagnostic, DiagnosticCode};

/// 正整数模数（`m > 1`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Modulus {
    value: BigInt,
}

impl Modulus {
    /// 由已解码整数构造；`m ≤ 1` → `ATHENA_MODULUS_INVALID`。
    pub fn new(value: impl Into<BigInt>) -> Result<Self, Diagnostic> {
        let value = value.into();
        if value <= BigInt::one() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid).detail("value", value.to_string()));
        }
        Ok(Self { value })
    }

    /// 模数值（始终 `> 1`）。
    pub fn value(&self) -> &BigInt {
        &self.value
    }

    /// 将整数规范到 `[0, m)`。
    pub fn reduce(&self, n: &BigInt) -> BigInt {
        let mut r = n % &self.value;
        if r.is_negative() {
            r += &self.value;
        }
        r
    }
}

/// 绑定模数的剩余类代表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModularValue {
    /// 规范剩余 `[0, modulus)`。
    residue: BigInt,
    /// 绑定模数。
    modulus: Modulus,
}

impl ModularValue {
    /// 在给定模数下构造（自动化约）。
    pub fn new(residue: impl Into<BigInt>, modulus: Modulus) -> Self {
        let residue = modulus.reduce(&residue.into());
        Self { residue, modulus }
    }

    /// 剩余。
    pub fn residue(&self) -> &BigInt {
        &self.residue
    }

    /// 模数。
    pub fn modulus(&self) -> &Modulus {
        &self.modulus
    }

    /// 同模运算前置检查。
    pub fn same_modulus(&self, other: &Self) -> Result<(), Diagnostic> {
        if self.modulus == other.modulus {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .arg("left_modulus", self.modulus.value().to_string())
                .arg("right_modulus", other.modulus.value().to_string())
                .detail("operation", "modular_binop"))
        }
    }
}
