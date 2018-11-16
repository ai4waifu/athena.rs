//! 𝔽_p 系数专用内核：小素数 word path · 大素数 `Modulus` path。

use athena_numeric::{Integer, Modulus, Number, NumericValue};
use athena_types::{Diagnostic, DiagnosticCode, Result};

/// 小素数 word 内核（`p` 落入 `u64`；乘积经 `u128` 约化）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FpWordKernel {
    p: u64,
}

impl FpWordKernel {
    /// 由已验证素数（`2 ≤ p ≤ u64::MAX`）构造。
    pub fn new(p: u64) -> Result<Self> {
        if p < 2 {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "polynomial")
                .detail("operation", "fp_word_prime"));
        }
        Ok(Self { p })
    }

    /// 特征素数。
    pub fn characteristic(&self) -> u64 {
        self.p
    }

    /// 系数加法。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        let x = self.reduce_number(&a)?;
        let y = self.reduce_number(&b)?;
        Ok(Number::integer(Integer::from_u64(add_mod(x, y, self.p))))
    }

    /// 系数乘法。
    pub fn mul(&self, a: Number, b: Number) -> Result<Number> {
        let x = self.reduce_number(&a)?;
        let y = self.reduce_number(&b)?;
        Ok(Number::integer(Integer::from_u64(mul_mod(x, y, self.p))))
    }

    /// 系数取负。
    pub fn neg(&self, a: Number) -> Result<Number> {
        let x = self.reduce_number(&a)?;
        Ok(Number::integer(Integer::from_u64(neg_mod(x, self.p))))
    }

    /// 素域是域。
    pub fn is_field(&self) -> bool {
        true
    }

    /// 域除法。
    pub fn div(&self, a: Number, b: Number) -> Result<Number> {
        let y = self.reduce_number(&b)?;
        if y == 0 {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "polynomial"));
        }
        let inv = inv_mod(y, self.p)?;
        let x = self.reduce_number(&a)?;
        Ok(Number::integer(Integer::from_u64(mul_mod(x, inv, self.p))))
    }

    /// 乘法逆元。
    pub fn inv(&self, a: Number) -> Result<Number> {
        self.div(Number::small_int(1), a)
    }

    fn reduce_number(&self, coeff: &Number) -> Result<u64> {
        let integer = extract_integer(coeff)?;
        Ok(reduce_i_to_u64(&integer, self.p))
    }
}

/// 大素数内核（通用 [`Modulus`] 约化）。
#[derive(Debug, Clone)]
pub struct FpBigKernel {
    modulus: Modulus,
}

impl FpBigKernel {
    /// 由已验证素数模数构造。
    pub fn new(modulus: Modulus) -> Self {
        Self { modulus }
    }

    /// 特征素数。
    pub fn characteristic(&self) -> &Integer {
        self.modulus.value()
    }

    /// 系数加法。
    pub fn add(&self, a: Number, b: Number) -> Result<Number> {
        self.reduce(athena_numeric::add(a, b)?)
    }

    /// 系数乘法。
    pub fn mul(&self, a: Number, b: Number) -> Result<Number> {
        self.reduce(athena_numeric::mul(a, b)?)
    }

    /// 系数取负。
    pub fn neg(&self, a: Number) -> Result<Number> {
        self.reduce(athena_numeric::neg(a))
    }

    /// 素域是域。
    pub fn is_field(&self) -> bool {
        true
    }

    /// 域除法。
    pub fn div(&self, a: Number, b: Number) -> Result<Number> {
        if b.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "polynomial"));
        }
        let bi = extract_integer(&b)?;
        let inv = crate::number_theory::mod_inverse(&bi, &self.modulus)?;
        self.mul(a, Number::integer(inv.residue().clone()))
    }

    /// 乘法逆元。
    pub fn inv(&self, a: Number) -> Result<Number> {
        self.div(Number::small_int(1), a)
    }

    fn reduce(&self, coeff: Number) -> Result<Number> {
        let integer = extract_integer(&coeff)?;
        Ok(Number::integer(self.modulus.reduce(&integer)))
    }
}

/// 由素数模数选择 word / big 内核。
pub fn select_fp_kernel(modulus: Modulus) -> Result<FpKernelKind> {
    match modulus.value().to_u64() {
        Some(p) if p >= 2 => Ok(FpKernelKind::Word(FpWordKernel::new(p)?)),
        _ => Ok(FpKernelKind::Big(FpBigKernel::new(modulus))),
    }
}

/// 𝔽_p 内核变体（intern 时选定）。
#[derive(Debug, Clone)]
pub enum FpKernelKind {
    /// `u64` word path。
    Word(FpWordKernel),
    /// 大素数 `Modulus` path。
    Big(FpBigKernel),
}

fn extract_integer(coeff: &Number) -> Result<Integer> {
    match coeff {
        NumericValue::Integer(i) => Ok(i.clone()),
        _ => Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "polynomial")
            .detail("operation", "coeff_integer_required")),
    }
}

fn reduce_i_to_u64(n: &Integer, p: u64) -> u64 {
    // 优先小整数快路径。
    if let Some(v) = n.to_i64() {
        let p_i = p as i64;
        let mut r = v % p_i;
        if r < 0 {
            r += p_i;
        }
        return r as u64;
    }
    let m = Integer::from_u64(p);
    let r = {
        let mut r = n.rem(&m);
        if r.is_negative() {
            r = r.add(&m);
        }
        r
    };
    r.to_u64().expect("residue in [0, p)")
}

fn add_mod(a: u64, b: u64, p: u64) -> u64 {
    ((u128::from(a) + u128::from(b)) % u128::from(p)) as u64
}

fn mul_mod(a: u64, b: u64, p: u64) -> u64 {
    ((u128::from(a) * u128::from(b)) % u128::from(p)) as u64
}

fn neg_mod(a: u64, p: u64) -> u64 {
    if a == 0 { 0 } else { p - a }
}

fn inv_mod(a: u64, p: u64) -> Result<u64> {
    // Extended Euclid on i128 for signed intermediates.
    let mut t: i128 = 0;
    let mut new_t: i128 = 1;
    let mut r: i128 = p as i128;
    let mut new_r: i128 = a as i128;
    while new_r != 0 {
        let q = r / new_r;
        (t, new_t) = (new_t, t - q * new_t);
        (r, new_r) = (new_r, r - q * new_r);
    }
    if r > 1 {
        return Err(Diagnostic::new(DiagnosticCode::ModularInverseMissing)
            .detail("domain", "polynomial")
            .detail("operation", "fp_word_inv")
            .detail("residue", a.to_string())
            .detail("modulus", p.to_string()));
    }
    if t < 0 {
        t += p as i128;
    }
    Ok(t as u64)
}
