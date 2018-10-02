//! 模数与模整数（numeric 层；公共面为 [`Integer`]，不暴露 `num_bigint`）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::integer::Integer;

/// 正整数模数（`m > 1`）。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Modulus {
    value: Integer,
}

impl Modulus {
    /// 由精确整数构造；`m ≤ 1` → `ATHENA_MODULUS_INVALID`。
    pub fn new(value: impl Into<Integer>) -> Result<Self> {
        let value = value.into();
        if !value.is_positive() || value.is_one() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid).detail("value", value.to_decimal_string()));
        }
        Ok(Self { value })
    }

    /// 模数值（始终 `> 1`）。
    pub fn value(&self) -> &Integer {
        &self.value
    }

    /// 将整数规范到 `[0, m)`。
    pub fn reduce(&self, n: &Integer) -> Integer {
        let mut r = n.rem(&self.value);
        if r.is_negative() {
            r = r.add(&self.value);
        }
        r
    }
}

/// 绑定模数的剩余类代表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModularValue {
    /// 规范剩余 `[0, modulus)`。
    residue: Integer,
    /// 绑定模数。
    modulus: Modulus,
}

impl ModularValue {
    /// 在给定模数下构造（自动化约）。
    pub fn new(residue: impl Into<Integer>, modulus: Modulus) -> Self {
        let residue = modulus.reduce(&residue.into());
        Self { residue, modulus }
    }

    /// 剩余。
    pub fn residue(&self) -> &Integer {
        &self.residue
    }

    /// 模数。
    pub fn modulus(&self) -> &Modulus {
        &self.modulus
    }

    /// 同模运算前置检查。
    pub fn same_modulus(&self, other: &Self) -> Result<()> {
        if self.modulus == other.modulus {
            Ok(())
        }
        else {
            Err(Diagnostic::new(DiagnosticCode::DomainMismatch)
                .arg("left_modulus", self.modulus.value().to_decimal_string())
                .arg("right_modulus", other.modulus.value().to_decimal_string())
                .detail("operation", "modular_binop"))
        }
    }
}
