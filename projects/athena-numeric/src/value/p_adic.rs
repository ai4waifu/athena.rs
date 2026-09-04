//! 固定精度 `ℚ_p`（模 `pⁿ` 截断）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::{execution_budget::NumericContext, integer::Integer, rational::Rational};

/// p-adic 截断值：小端 `p`-进制 digits，长度 `≤ precision`。
#[derive(Debug, PartialEq, Eq)]
pub struct PAdicValue {
    /// 素数 `p`。
    pub prime: Integer,
    /// 精度 `n`（模 `pⁿ`）。
    pub precision: u32,
    /// 小端 digits，每位 `0 ≤ d_i < p`。
    pub digits: Vec<u32>,
}

impl PAdicValue {
    /// Owning 深复制（Living `19`）。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        Ok(Self { prime: self.prime.try_clone_in(ctx)?, precision: self.precision, digits: self.digits.clone() })
    }

    /// 校验并规范化构造。
    pub fn try_new(prime: Integer, precision: u32, digits: Vec<u32>) -> Result<Self> {
        let v = Self { prime, precision, digits };
        v.validate()?;
        Ok(v.normalized())
    }

    /// 从整数嵌入（截断至 `pⁿ`）。
    pub fn from_integer(n: &Integer, prime: Integer, precision: u32) -> Result<Self> {
        validate_prime_precision(&prime, precision)?;
        let modulus = pow_prime(&prime, precision)?;
        let r = n.rem_euclid(&modulus)?;
        Ok(from_residue(r, prime, precision))
    }

    /// 从有理嵌入：分母须与 `p` 互素。
    pub fn from_rational(r: &Rational, prime: Integer, precision: u32) -> Result<Self> {
        validate_prime_precision(&prime, precision)?;
        let den = r.denominator();
        if den.gcd(&prime) != Integer::one() {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "padic_from_rational_denominator"));
        }
        let modulus = pow_prime(&prime, precision)?;
        let inv_den = inv_mod(&den, &modulus).ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "numeric").detail("operation", "padic_from_rational_inv")
        })?;
        let num = r.numerator().rem_euclid(&modulus)?;
        let residue = num.mul(&inv_den).rem_euclid(&modulus)?;
        Ok(from_residue(residue, prime, precision))
    }

    /// 不变量校验。
    pub fn validate(&self) -> Result<()> {
        validate_prime_precision(&self.prime, self.precision)?;
        let p = self.prime.to_u64().ok_or_else(|| {
            Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "numeric").detail("operation", "padic_prime_too_large")
        })?;
        if self.digits.len() > self.precision as usize {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "padic_digits_len"));
        }
        for &d in &self.digits {
            if u64::from(d) >= p {
                return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                    .detail("domain", "numeric")
                    .detail("operation", "padic_digit_range"));
            }
        }
        Ok(())
    }

    /// 截断到更小精度。
    pub fn truncate(&self, new_precision: u32) -> Result<Self> {
        if new_precision == 0 || new_precision > self.precision {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
                .detail("domain", "numeric")
                .detail("operation", "padic_truncate"));
        }
        let mut digits = self.digits.clone();
        digits.truncate(new_precision as usize);
        Self::try_new(self.prime.try_clone_in(&NumericContext::portable_default())?, new_precision, digits)
    }

    /// 零扩展到更大精度（同 `p`）。
    pub fn lift(&self, new_precision: u32) -> Result<Self> {
        if new_precision < self.precision {
            return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "numeric").detail("operation", "padic_lift"));
        }
        Self::try_new(self.prime.try_clone_in(&NumericContext::portable_default())?, new_precision, self.digits.clone())
    }

    /// 加法（同 `p`、同精度）。
    pub fn add(&self, other: &Self) -> Result<Self> {
        same_domain(self, other)?;
        let m = pow_prime(&self.prime, self.precision)?;
        let r = self.residue().add(&other.residue()).rem_euclid(&m)?;
        Ok(from_residue(normalize_mod(r, &m), self.prime.try_clone_in(&NumericContext::portable_default())?, self.precision))
    }

    /// 减法。
    pub fn sub(&self, other: &Self) -> Result<Self> {
        same_domain(self, other)?;
        let m = pow_prime(&self.prime, self.precision)?;
        let r = self.residue().sub(&other.residue()).rem_euclid(&m)?;
        Ok(from_residue(normalize_mod(r, &m), self.prime.try_clone_in(&NumericContext::portable_default())?, self.precision))
    }

    /// 乘法。
    pub fn mul(&self, other: &Self) -> Result<Self> {
        same_domain(self, other)?;
        let m = pow_prime(&self.prime, self.precision)?;
        let r = self.residue().mul(&other.residue()).rem_euclid(&m)?;
        Ok(from_residue(normalize_mod(r, &m), self.prime.try_clone_in(&NumericContext::portable_default())?, self.precision))
    }

    /// 取负。
    pub fn neg(&self) -> Result<Self> {
        let m = pow_prime(&self.prime, self.precision)?;
        let r = self.residue().neg().rem_euclid(&m)?;
        Ok(from_residue(normalize_mod(r, &m), self.prime.try_clone_in(&NumericContext::portable_default())?, self.precision))
    }

    /// 逆元（须为 `p`-adic 单位）。
    pub fn inv(&self) -> Result<Self> {
        if !self.is_unit() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "numeric").detail("operation", "padic_inv_non_unit"));
        }
        let m = pow_prime(&self.prime, self.precision)?;
        let inv = inv_mod(&self.residue(), &m)
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::DivideByZero).detail("domain", "numeric").detail("operation", "padic_inv"))?;
        Ok(from_residue(inv, self.prime.try_clone_in(&NumericContext::portable_default())?, self.precision))
    }

    /// 是否为 `p`-adic 单位（`vₚ = 0`）。
    pub fn is_unit(&self) -> bool {
        match self.digits.first() {
            Some(&d) => d != 0,
            None => false,
        }
    }

    /// 是否为零（模 `pⁿ`）。
    pub fn is_zero(&self) -> bool {
        self.digits.iter().all(|&d| d == 0)
    }

    fn residue(&self) -> Integer {
        let mut acc = Integer::zero();
        let mut pow = Integer::one();
        for &d in &self.digits {
            acc = acc.add(&Integer::from_u64(u64::from(d)).mul(&pow));
            pow = pow.mul(&self.prime);
        }
        acc
    }

    fn normalized(self) -> Self {
        let mut digits = self.digits;
        while digits.last() == Some(&0) {
            digits.pop();
        }
        Self { digits, ..self }
    }
}

fn same_domain(a: &PAdicValue, b: &PAdicValue) -> Result<()> {
    if a.prime != b.prime || a.precision != b.precision {
        return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "numeric").detail("operation", "padic_domain"));
    }
    Ok(())
}

fn validate_prime_precision(prime: &Integer, precision: u32) -> Result<()> {
    if precision == 0 {
        return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "numeric")
            .detail("operation", "padic_precision_zero"));
    }
    if !prime.is_positive() {
        return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "numeric")
            .detail("operation", "padic_prime_non_positive"));
    }
    let Some(p) = prime.to_u64()
    else {
        return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "numeric")
            .detail("operation", "padic_prime_too_large"));
    };
    if p < 2 || !is_prime_u64(p) {
        return Err(Diagnostic::new(DiagnosticCode::NumericDomainMismatch)
            .detail("domain", "numeric")
            .detail("operation", "padic_prime_composite"));
    }
    Ok(())
}

fn is_prime_u64(n: u64) -> bool {
    if n < 2 {
        return false;
    }
    if n % 2 == 0 {
        return n == 2;
    }
    let mut d = 3u64;
    while d.saturating_mul(d) <= n {
        if n % d == 0 {
            return false;
        }
        d += 2;
    }
    true
}

fn pow_prime(prime: &Integer, n: u32) -> Result<Integer> {
    prime
        .pow_u32(n)
        .map_err(|_| Diagnostic::new(DiagnosticCode::NumericDomainMismatch).detail("domain", "numeric").detail("operation", "padic_pow"))
}

fn normalize_mod(r: Integer, m: &Integer) -> Integer {
    r.rem_euclid(m).expect("padic modulus")
}

fn from_residue(mut r: Integer, prime: Integer, precision: u32) -> PAdicValue {
    let mut digits = Vec::new();
    for _ in 0..precision {
        if r.is_zero() {
            break;
        }
        let digit = r.rem_euclid(&prime).expect("prime");
        let d = digit.to_u64().unwrap_or(0) as u32;
        digits.push(d);
        r = r.div(&prime).expect("prime");
    }
    PAdicValue { prime, precision, digits }.normalized()
}

fn inv_mod(a: &Integer, m: &Integer) -> Option<Integer> {
    if a.gcd(m) != Integer::one() {
        return None;
    }
    let mut t = Integer::zero();
    let mut newt = Integer::one();
    let mut r = m.try_clone_in(&NumericContext::portable_default()).ok()?;
    let mut newr = a.rem_euclid(m).ok()?;
    while !newr.is_zero() {
        let q = r.div(&newr).ok()?;
        let tmp_t = t.sub(&q.mul(&newt));
        t = newt;
        newt = tmp_t;
        let tmp_r = r.sub(&q.mul(&newr));
        r = newr;
        newr = tmp_r;
    }
    Some(t.rem_euclid(m).ok()?)
}
