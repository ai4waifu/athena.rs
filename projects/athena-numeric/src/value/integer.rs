//! 精确整数（自有 `meta + Magnitude`；`Sign` 仅为语义 API）。

use athena_types::{Diagnostic, DiagnosticCode, Result};
use std::str::FromStr;

use crate::magnitude::MagnitudePair;
use crate::{execution_budget::NumericContext, natural::Natural};

/// 符号（语义 API；不作为 [`Integer`] 存储字段）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Sign {
    /// 负。
    Negative,
    /// 零。
    Zero,
    /// 正。
    Positive,
}

/// 精确整数（稳定公共包装；亦称 [`ExactInteger`]）。
///
/// 布局：`meta`（mode+sign+heap_len）+ `union Magnitude`，LP64 上 24 bytes。
/// 经私有 [`MagnitudePair`] 做 Drop/Clone；无独立 `Sign` 字段、不嵌套 `Natural`。
/// 排序必须是数学序：负数额值反序、正数额值正序。禁止 derive `Ord`。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Integer {
    inner: MagnitudePair,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Integer>() == 24);
    assert!(core::mem::align_of::<Integer>() == 8);
};

impl PartialOrd for Integer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Integer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self.sign(), other.sign()) {
            (Sign::Negative, Sign::Positive) | (Sign::Negative, Sign::Zero) => Ordering::Less,
            (Sign::Positive, Sign::Negative) | (Sign::Zero, Sign::Negative) => Ordering::Greater,
            (Sign::Zero, Sign::Zero) => Ordering::Equal,
            (Sign::Zero, Sign::Positive) => Ordering::Less,
            (Sign::Positive, Sign::Zero) => Ordering::Greater,
            (Sign::Positive, Sign::Positive) => self.inner.as_limbs().cmp(other.inner.as_limbs()),
            (Sign::Negative, Sign::Negative) => other.inner.as_limbs().cmp(self.inner.as_limbs()),
        }
    }
}

/// 稳定别名（与 [`NumericValue`] 同义迁移期命名）。
pub type ExactInteger = Integer;

impl Integer {
    fn from_mag_sign(mag: Natural, negative: bool) -> Self {
        Self { inner: mag.into_pair().with_negative(negative) }
    }

    pub(crate) fn from_pair(inner: MagnitudePair) -> Self {
        Self { inner }
    }

    pub(crate) fn into_pair(self) -> MagnitudePair {
        self.inner
    }

    /// 无符号幅度（克隆；供模运算等）。
    fn abs_natural(&self) -> Natural {
        Natural::from_pair(self.inner.clone_clear_sign())
    }

    /// 由已解码 `i64` 构造。
    pub fn from_i64(n: i64) -> Self {
        if n == 0 {
            Self::zero()
        } else if n < 0 {
            Self::from_mag_sign(Natural::from_u64(n.unsigned_abs()), true)
        } else {
            Self::from_mag_sign(Natural::from_u64(n as u64), false)
        }
    }

    /// 由 `u64` 构造。
    pub fn from_u64(n: u64) -> Self {
        Self::from_mag_sign(Natural::from_u64(n), false)
    }

    /// 零。
    pub fn zero() -> Self {
        Self { inner: MagnitudePair::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self::from_mag_sign(Natural::one(), false)
    }

    /// 非负幅度（crate 内部；模内核用）。
    pub(crate) fn magnitude(&self) -> Natural {
        self.abs_natural()
    }

    /// 由非负 [`Natural`] 构造（crate 内部）。
    pub(crate) fn from_positive_natural(mag: Natural) -> Self {
        Self::from_mag_sign(mag, false)
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        matches!(self.sign(), Sign::Positive) && self.inner.as_limbs() == [1]
    }

    /// 是否为负。
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// 是否为正。
    pub fn is_positive(&self) -> bool {
        matches!(self.sign(), Sign::Positive)
    }

    /// 是否非负（零视为非负）。
    pub fn is_non_negative(&self) -> bool {
        !self.is_negative()
    }

    /// 符号（由 `meta` 解码；零无负零）。
    pub fn sign(&self) -> Sign {
        if self.inner.is_zero() {
            Sign::Zero
        } else if self.inner.is_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    /// 绝对值。
    pub fn abs(&self) -> Self {
        Self::from_pair(self.inner.clone_clear_sign())
    }

    /// 取负。
    pub fn neg(&self) -> Self {
        match self.sign() {
            Sign::Zero => Self::zero(),
            Sign::Positive => Self::from_pair(self.inner.clone().with_negative(true)),
            Sign::Negative => Self::from_pair(self.inner.clone_clear_sign()),
        }
    }

    /// 非负最大公约数；`gcd(0,0) = 0`（默认上下文）。
    pub fn gcd(&self, other: &Self) -> Self {
        self.try_gcd(other, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 非负最大公约数（服从 `ctx` 预算）。
    pub fn try_gcd(&self, other: &Self, ctx: &NumericContext) -> Result<Self> {
        let a = self.abs_natural();
        let b = other.abs_natural();
        if a.is_zero() && b.is_zero() {
            return Ok(Self::zero());
        }
        let g = Natural::try_gcd(&a, &b, ctx)?;
        Ok(Self::from_positive_natural(g))
    }

    /// 加法（默认 [`NumericContext::pure_rust_default`]）。
    pub fn add(&self, rhs: &Self) -> Self {
        self.try_add(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 加法（服从 `ctx` 预算）。
    pub fn try_add(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        Ok(match (self.sign(), rhs.sign()) {
            (Sign::Zero, _) => rhs.clone(),
            (_, Sign::Zero) => self.clone(),
            (Sign::Positive, Sign::Positive) => {
                Self::from_positive_natural(self.abs_natural().try_add(&rhs.abs_natural(), ctx)?)
            }
            (Sign::Negative, Sign::Negative) => {
                Self::from_mag_sign(self.abs_natural().try_add(&rhs.abs_natural(), ctx)?, true)
            }
            (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive) => {
                let sa = self.abs_natural();
                let sb = rhs.abs_natural();
                if sa >= sb {
                    let mag = sa.try_sub(&sb, ctx)?;
                    Self::from_mag_sign(mag, self.is_negative())
                } else {
                    let mag = sb.try_sub(&sa, ctx)?;
                    Self::from_mag_sign(mag, rhs.is_negative())
                }
            }
        })
    }

    /// 减法（默认上下文）。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.neg())
    }

    /// 减法（服从 `ctx` 预算）。
    pub fn try_sub(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        self.try_add(&rhs.neg(), ctx)
    }

    /// 乘法（默认上下文）。
    pub fn mul(&self, rhs: &Self) -> Self {
        self.try_mul(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 乘法（服从 `ctx` 预算）。
    pub fn try_mul(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let negative = self.is_negative() != rhs.is_negative();
        Ok(Self::from_mag_sign(
            self.abs_natural().try_mul(&rhs.abs_natural(), ctx)?,
            negative,
        ))
    }

    /// 向零整除：商向零，余数与被除数同号（默认上下文）。
    pub fn div_rem_trunc(&self, rhs: &Self) -> Result<(Self, Self)> {
        self.try_div_rem_trunc(rhs, &NumericContext::pure_rust_default())
    }

    /// 向零整除（服从 `ctx` 预算）。
    pub fn try_div_rem_trunc(&self, rhs: &Self, ctx: &NumericContext) -> Result<(Self, Self)> {
        if rhs.is_zero() {
            return Err(division_by_zero("div_rem_trunc"));
        }
        let (q_mag, r_mag) = self.abs_natural().try_div_rem(&rhs.abs_natural(), ctx)?;
        let q_neg = self.is_negative() != rhs.is_negative();
        let r_neg = self.is_negative();
        Ok((Self::from_mag_sign(q_mag, q_neg), Self::from_mag_sign(r_mag, r_neg)))
    }

    /// Euclidean 整除：余数满足 `0 <= r < |rhs|`（默认上下文）。
    pub fn div_rem_euclid(&self, rhs: &Self) -> Result<(Self, Self)> {
        self.try_div_rem_euclid(rhs, &NumericContext::pure_rust_default())
    }

    /// Euclidean 整除（服从 `ctx` 预算）。
    pub fn try_div_rem_euclid(&self, rhs: &Self, ctx: &NumericContext) -> Result<(Self, Self)> {
        if rhs.is_zero() {
            return Err(division_by_zero("div_rem_euclid"));
        }
        let (mut q, mut r) = self.try_div_rem_trunc(rhs, ctx)?;
        if r.is_negative() {
            if rhs.is_positive() {
                r = r.try_add(rhs, ctx)?;
                q = q.try_sub(&Self::one(), ctx)?;
            } else {
                r = r.try_sub(rhs, ctx)?;
                q = q.try_add(&Self::one(), ctx)?;
            }
        }
        debug_assert!(!r.is_negative());
        debug_assert!(r.abs_natural() < rhs.abs_natural() || r.is_zero());
        Ok((q, r))
    }

    /// 向零整除商（见 [`Self::div_rem_trunc`]）。
    pub fn div(&self, rhs: &Self) -> Result<Self> {
        Ok(self.div_rem_trunc(rhs)?.0)
    }

    /// 向零整除余数（见 [`Self::div_rem_trunc`]）。语言级 truncating rem，**不是** Euclidean 模。
    pub fn rem(&self, rhs: &Self) -> Result<Self> {
        Ok(self.div_rem_trunc(rhs)?.1)
    }

    /// Euclidean 余数：`0 <= r < |rhs|`。
    pub fn rem_euclid(&self, rhs: &Self) -> Result<Self> {
        Ok(self.div_rem_euclid(rhs)?.1)
    }

    /// 模幂：`self^exp mod modulus`（`modulus` 须为正；底数经 Euclidean 归约）。
    ///
    /// 负指数暂不支持（返回诊断，不静默返零）。
    pub fn mod_pow(&self, exp: &Self, modulus: &Self) -> Result<Self> {
        self.try_mod_pow(exp, modulus, &NumericContext::pure_rust_default())
    }

    /// 模幂（服从 `ctx` 预算）。
    pub fn try_mod_pow(&self, exp: &Self, modulus: &Self, ctx: &NumericContext) -> Result<Self> {
        if !modulus.is_positive() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "numeric")
                .detail("operation", "mod_pow")
                .detail("reason", "modulus_not_positive"));
        }
        if exp.is_negative() {
            return Err(Diagnostic::new(DiagnosticCode::UnsupportedOperation)
                .detail("domain", "numeric")
                .detail("operation", "mod_pow")
                .detail("reason", "negative_exponent_requires_modular_inverse"));
        }
        let base = self.try_div_rem_euclid(modulus, ctx)?.1;
        let result_mag = base
            .abs_natural()
            .try_mod_pow(&exp.abs_natural(), &modulus.abs_natural(), ctx)?;
        Ok(Self::from_positive_natural(result_mag))
    }

    /// 绝对值的二进制位宽（`0` → `0`）。
    pub fn bits(&self) -> u64 {
        self.abs_natural().bits()
    }

    /// 可无损落入 `i64` 时返回（含 `i64::MIN` 与零）。
    pub fn to_i64(&self) -> Option<i64> {
        if self.is_zero() {
            return Some(0);
        }
        let u = self.abs_natural().to_u128()?;
        let wide = match self.sign() {
            Sign::Zero => 0_i128,
            Sign::Positive => i128::try_from(u).ok()?,
            Sign::Negative => {
                if u > 1_u128 << 63 {
                    return None;
                }
                -(u as i128)
            }
        };
        i64::try_from(wide).ok()
    }

    /// 可无损落入 `u64` 时返回（零 → `Some(0)`）。
    pub fn to_u64(&self) -> Option<u64> {
        if self.is_zero() {
            return Some(0);
        }
        match self.sign() {
            Sign::Positive => self.abs_natural().to_u64(),
            Sign::Negative | Sign::Zero => None,
        }
    }

    /// 可无损落入 `u128` 时返回（零 → `Some(0)`）。
    pub fn to_u128(&self) -> Option<u128> {
        if self.is_zero() {
            return Some(0);
        }
        match self.sign() {
            Sign::Positive => self.abs_natural().to_u128(),
            Sign::Negative | Sign::Zero => None,
        }
    }

    /// IEEE binary64 可精确表示的整数绝对值上限（`2^53`）。
    const F64_EXACT_ABS_MAX: u128 = 1u128 << 53;

    /// 仅当值在 binary64 上可精确表示时返回（可逆）。
    pub fn try_to_f64_exact(&self) -> Option<f64> {
        if self.is_zero() {
            return Some(0.0);
        }
        let u = self.abs_natural().to_u128()?;
        if u > Self::F64_EXACT_ABS_MAX {
            return None;
        }
        let f = if self.is_negative() { -(u as f64) } else { u as f64 };
        if !f.is_finite() {
            return None;
        }
        if f64_represents_integer(f, self) {
            Some(f)
        } else {
            None
        }
    }

    /// 明确近似的 `f64`（不保证可逆；宿主桥接用）。
    pub fn to_f64_approximate(&self) -> Option<f64> {
        if let Some(i) = self.to_i64() {
            return Some(i as f64);
        }
        self.to_decimal_string().parse::<f64>().ok().filter(|x| x.is_finite())
    }

    /// 同 [`try_to_f64_exact`]（过渡期别名）。
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        self.try_to_f64_exact()
    }

    /// 十进制调试字符串（非本地化用户文案）。
    pub fn to_decimal_string(&self) -> String {
        match self.sign() {
            Sign::Zero => "0".to_string(),
            Sign::Positive => self.abs_natural().to_decimal_string(),
            Sign::Negative => format!("-{}", self.abs_natural().to_decimal_string()),
        }
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        self.is_positive() && self.abs_natural().is_power_of_two()
    }

    /// 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && self.abs_natural().is_odd()
    }

    /// 非负 `u32` 指数幂（独立实现，不回调 [`pow`]）。
    pub fn pow_u32(&self, exp: u32) -> std::result::Result<Self, ()> {
        if exp as i64 > Self::MAX_POW_EXP {
            return Err(());
        }
        if exp == 0 {
            return Ok(Self::one());
        }
        if self.is_zero() {
            return Ok(Self::zero());
        }
        let mut acc = Self::one();
        let mut base = self.clone();
        let mut e = exp;
        while e > 0 {
            if e & 1 == 1 {
                acc = acc.mul(&base);
            }
            base = base.mul(&base);
            e >>= 1;
        }
        Ok(acc)
    }

    /// 非负整数幂允许的最大指数（与阶乘等资源合同一致）。
    pub const MAX_POW_EXP: i64 = 10_000;

    /// 非负整数幂（二进制幂；指数须 `>= 0` 且 `<= MAX_POW_EXP`）。
    pub fn pow(&self, exp: &Integer) -> std::result::Result<Self, ()> {
        if exp.is_negative() {
            return Err(());
        }
        if exp.is_zero() {
            return Ok(Self::one());
        }
        if self.is_zero() {
            return Ok(Self::zero());
        }
        if let Some(e) = exp.to_i64() {
            if e > Self::MAX_POW_EXP {
                return Err(());
            }
        } else {
            return Err(());
        }
        let mut acc = Self::one();
        let mut base = self.clone();
        let mut e = exp.clone();
        let two = Integer::from_i64(2);
        while !e.is_zero() {
            if e.is_odd() {
                acc = acc.mul(&base);
            }
            base = base.mul(&base);
            e = e.div(&two).map_err(|_| ())?;
        }
        Ok(acc)
    }

    /// 非负整数平方根（向下取整）。负数返回域错误，不静默返零。
    pub fn int_sqrt(&self) -> Result<Self> {
        if self.is_negative() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "int_sqrt")
                .detail("reason", "negative"));
        }
        if self.is_zero() {
            return Ok(Self::zero());
        }
        let mut lo = Self::zero();
        let mut hi = self.clone().add(&Self::one());
        let two = Integer::from_i64(2);
        while lo.add(&Integer::one()).cmp(&hi) == std::cmp::Ordering::Less {
            let mid = lo.add(&hi).div(&two).expect("divisor two");
            if mid.mul(&mid) <= *self {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        Ok(lo)
    }

    /// 二进制 wire 符号码：`0` 零 · `1` 正 · `2` 负。
    pub(crate) fn wire_sign_code(&self) -> u8 {
        match self.sign() {
            Sign::Zero => 0,
            Sign::Positive => 1,
            Sign::Negative => 2,
        }
    }

    /// 二进制 wire 的无符号幅度字节。
    pub(crate) fn wire_magnitude_bytes(&self) -> Vec<u8> {
        self.abs_natural().wire_encode_magnitude()
    }

    /// 由符号码 + 幅度字节解码二进制 wire 整数。
    pub(crate) fn from_wire_magnitude(sign: u8, mag_bytes: &[u8]) -> Result<Self> {
        let mag = Natural::wire_decode_magnitude(mag_bytes)?;
        Self::from_wire_parts(sign, mag)
    }

    /// 由符号码 + 已解码幅度解码（canonical：`sign=0` ⇔ mag 为零；禁止负零）。
    pub(crate) fn from_wire_parts(sign: u8, mag: Natural) -> Result<Self> {
        use crate::format::validation::{reject_non_canonical, WireReject};
        match sign {
            0 => {
                if !mag.is_zero() {
                    return Err(reject_non_canonical(WireReject::SignZeroNonzeroMag));
                }
                Ok(Self::zero())
            }
            1 => {
                if mag.is_zero() {
                    return Err(reject_non_canonical(WireReject::SignPosZeroMag));
                }
                Ok(Self::from_mag_sign(mag, false))
            }
            2 => {
                if mag.is_zero() {
                    return Err(reject_non_canonical(WireReject::SignNegZeroMag));
                }
                Ok(Self::from_mag_sign(mag, true))
            }
            _ => Err(reject_non_canonical(WireReject::SignUnknown)),
        }
    }
}

fn division_by_zero(op: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticCode::NumericDivisionByZero)
        .detail("domain", "numeric")
        .detail("operation", op)
}

fn f64_represents_integer(f: f64, n: &Integer) -> bool {
    if !f.is_finite() {
        return false;
    }
    if n.is_zero() {
        return f == 0.0;
    }
    let u = match n.abs_natural().to_u128() {
        Some(v) => v,
        None => return false,
    };
    let expected = if n.is_negative() { -(u as f64) } else { u as f64 };
    f.to_bits() == expected.to_bits()
}

impl From<i64> for Integer {
    fn from(n: i64) -> Self {
        Self::from_i64(n)
    }
}

impl From<i32> for Integer {
    fn from(n: i32) -> Self {
        Self::from_i64(i64::from(n))
    }
}

impl From<u32> for Integer {
    fn from(n: u32) -> Self {
        Self::from_u64(u64::from(n))
    }
}

impl From<u64> for Integer {
    fn from(n: u64) -> Self {
        Self::from_u64(n)
    }
}

impl FromStr for Integer {
    type Err = ();
    /// 解码规范十进制数字
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let t = s.trim();
        if t.is_empty() {
            return Err(());
        }
        let (negative, digits) = match t.as_bytes()[0] {
            b'+' => (false, &t[1..]),
            b'-' => (true, &t[1..]),
            _ => (false, t),
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let mag = Natural::from_str(digits)?;
        Ok(Self::from_mag_sign(mag, negative))
    }
}

impl athena_gc::Trace for Integer {
    fn trace(&self, tracer: &mut dyn athena_gc::Tracer) {
        if let Some(ptr) = self.inner.heap_ptr() {
            tracer.mark_allocation(ptr.as_ptr().cast());
        }
    }
}
