//! 非负大整数（`meta` + 纯 `union Magnitude`；算法委托 [`crate::kernel::limb`]）。

use athena_types::{Diagnostic, DiagnosticCode, Result};

use crate::magnitude::{Mode, MagnitudePair};
use crate::{kernel::limb as limb_kernel, policy::execution_budget::NumericContext};
use std::{cmp::Ordering, str::FromStr};

/// 自然数（小端 `u64` limb，无尾随零）。
///
/// 布局：`meta`（仅 mode+heap_len）+ `union Magnitude`，LP64 上 24 bytes。
/// 经私有 [`MagnitudePair`] 做 Drop/Clone；不解释 sign。
#[derive(Clone, PartialEq, Eq, Hash, Default)]
pub struct Natural {
    inner: MagnitudePair,
}

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Natural>() == 24);
    assert!(core::mem::align_of::<Natural>() == 8);
};

impl core::fmt::Debug for Natural {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Natural").field("limbs", &self.as_limbs()).finish()
    }
}

impl PartialOrd for Natural {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Natural {
    fn cmp(&self, other: &Self) -> Ordering {
        limb_kernel::cmp_slice(self.as_limbs(), other.as_limbs())
    }
}

impl Natural {
    /// 零。
    pub fn zero() -> Self {
        Self { inner: MagnitudePair::zero() }
    }

    /// 一。
    pub fn one() -> Self {
        Self { inner: MagnitudePair::from_u64(1) }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// 是否为一。
    pub fn is_one(&self) -> bool {
        matches!(self.inner.mode(), Mode::Limb1) && self.as_limbs() == [1]
    }

    /// 最低 limb 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && (self.as_limbs()[0] & 1) == 1
    }

    /// 由 `u64` 构造。
    pub fn from_u64(n: u64) -> Self {
        Self { inner: MagnitudePair::from_u64(n) }
    }

    /// 二进制位宽（零 → 0）。
    pub fn bits(&self) -> u64 {
        if self.is_zero() {
            return 0;
        }
        let limbs = self.as_limbs();
        let top = limbs.len() - 1;
        (top as u64) * 64 + (64 - limbs[top].leading_zeros() as u64)
    }

    /// 右移一位（整除 2）。
    pub fn div2(&mut self) {
        if self.is_zero() {
            return;
        }
        let mut limbs = self.as_limbs().to_vec();
        let mut carry = 0u64;
        let len = limb_kernel::effective_len(&limbs);
        for i in (0..len).rev() {
            let limb = limbs[i];
            let new_carry = limb & 1;
            limbs[i] = (limb >> 1) | (carry << 63);
            carry = new_carry;
        }
        *self = Self::from_limbs(limb_kernel::normalize_trim(limbs));
    }

    /// 右移 `n` 位（丢弃低位）。
    pub(crate) fn shr_bits(&self, n: u64) -> Self {
        if n == 0 || self.is_zero() {
            return self.clone();
        }
        let bits = self.bits();
        if n >= bits {
            return Self::zero();
        }
        let limb_shift = (n / 64) as usize;
        let bit_shift = (n % 64) as u32;
        let src = self.as_limbs();
        let el = src.len();
        if limb_shift >= el {
            return Self::zero();
        }
        let out_len = el - limb_shift;
        let mut out = vec![0u64; out_len];
        for i in 0..out_len {
            let src_i = i + limb_shift;
            out[i] = src[src_i] >> bit_shift;
            if bit_shift != 0 && src_i + 1 < el {
                out[i] |= src[src_i + 1] << (64 - bit_shift);
            }
        }
        Self::from_limbs(out)
    }

    /// 测试第 `index` 位（0 = LSB）。越界为 false。
    pub(crate) fn bit(&self, index: u64) -> bool {
        let limb_i = (index / 64) as usize;
        let limbs = self.as_limbs();
        if limb_i >= limbs.len() {
            return false;
        }
        let bit_i = (index % 64) as u32;
        (limbs[limb_i] >> bit_i) & 1 == 1
    }

    /// 是否有低于 `bit_index` 的任意置位（即 bits `[0, bit_index)`）。
    pub(crate) fn any_bits_below(&self, bit_index: u64) -> bool {
        if bit_index == 0 || self.is_zero() {
            return false;
        }
        let full_limbs = (bit_index / 64) as usize;
        let rem_bits = (bit_index % 64) as u32;
        let limbs = self.as_limbs();
        let el = limbs.len();
        let scan = full_limbs.min(el);
        for &limb in &limbs[..scan] {
            if limb != 0 {
                return true;
            }
        }
        if rem_bits > 0 && full_limbs < el {
            let mask = (1u64 << rem_bits) - 1;
            if limbs[full_limbs] & mask != 0 {
                return true;
            }
        }
        false
    }

    /// 加小整数（默认 [`NumericContext::pure_rust_default`]）。
    pub fn add_u64(&self, rhs: u64) -> Self {
        self.try_add_u64(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 加小整数（服从 `ctx` 预算）。
    pub fn try_add_u64(&self, rhs: u64, ctx: &NumericContext) -> Result<Self> {
        if rhs == 0 {
            return Ok(self.clone());
        }
        match self.inner.mode() {
            Mode::Zero => {
                ctx.budget().check_limbs(1)?;
                Ok(Self::from_u64(rhs))
            }
            Mode::Limb1 => {
                let a = self.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                let (lo, carry) = limb_kernel::add_1(a, rhs);
                Ok(if carry == 0 {
                    Self::from_u64(lo)
                } else {
                    Self { inner: MagnitudePair::from_limb2([lo, 1]) }
                })
            }
            Mode::Limb2 => {
                let a = self.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(rhs, a);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            Mode::Heap => {
                Ok(Self::from_limbs(limb_kernel::add_n_budgeted(self.as_limbs(), &[rhs], ctx.budget())?))
            }
        }
    }

    /// 乘小整数（默认 [`NumericContext::pure_rust_default`]）。
    pub fn mul_u64(&self, rhs: u64) -> Self {
        self.try_mul_u64(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 乘小整数（服从 `ctx` 预算）。
    pub fn try_mul_u64(&self, rhs: u64, ctx: &NumericContext) -> Result<Self> {
        if self.is_zero() || rhs == 0 {
            return Ok(Self::zero());
        }
        if rhs == 1 {
            return Ok(self.clone());
        }
        match self.inner.mode() {
            Mode::Zero => Ok(Self::zero()),
            Mode::Limb1 => {
                let a = self.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                Ok(Self { inner: MagnitudePair::from_u128(limb_kernel::mul_1x1(a, rhs)) })
            }
            Mode::Limb2 => {
                let a = self.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::mul_2x1(a, rhs);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            Mode::Heap => {
                Ok(Self::from_limbs(limb_kernel::mul_1_budgeted(self.as_limbs(), rhs, ctx.budget())?))
            }
        }
    }

    /// 加法（默认 [`NumericContext::pure_rust_default`]）。
    pub fn add(&self, rhs: &Self) -> Self {
        self.try_add(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 加法（服从 `ctx` 预算）。
    pub fn try_add(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        match (self.inner.mode(), rhs.inner.mode()) {
            (Mode::Zero, _) => Ok(rhs.clone()),
            (_, Mode::Zero) => Ok(self.clone()),
            (Mode::Limb1, Mode::Limb1) => {
                let a = self.inner.as_limb1().expect("Limb1");
                let b = rhs.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                let (lo, carry) = limb_kernel::add_1(a, b);
                Ok(if carry == 0 {
                    Self::from_u64(lo)
                } else {
                    Self { inner: MagnitudePair::from_limb2([lo, 1]) }
                })
            }
            (Mode::Limb1, Mode::Limb2) => {
                let a = self.inner.as_limb1().expect("Limb1");
                let b = rhs.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(a, b);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb1) => {
                let a = self.inner.as_limb2().expect("Limb2");
                let b = rhs.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_1_2(b, a);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb2) => {
                let a = self.inner.as_limb2().expect("Limb2");
                let b = rhs.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::add_2(a, b);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            _ => Ok(Self::from_limbs(limb_kernel::add_n_budgeted(
                self.as_limbs(),
                rhs.as_limbs(),
                ctx.budget(),
            )?)),
        }
    }

    /// 减法（要求 `self >= rhs`；默认上下文）。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.try_sub(rhs, &NumericContext::pure_rust_default())
            .expect("natural sub precondition or unbounded default")
    }

    /// 减法（`self >= rhs`；服从 `ctx` 预算）。
    pub fn try_sub(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        if self < rhs {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "natural_sub")
                .detail("reason", "underflow"));
        }
        Ok(Self::from_limbs(limb_kernel::sub_n_budgeted(self.as_limbs(), rhs.as_limbs(), ctx.budget())?))
    }

    /// 乘法（默认 [`NumericContext::pure_rust_default`]）。
    pub fn mul(&self, rhs: &Self) -> Self {
        self.try_mul(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 乘法（服从 `ctx` 预算）。
    pub fn try_mul(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        match (self.inner.mode(), rhs.inner.mode()) {
            (Mode::Zero, _) | (_, Mode::Zero) => Ok(Self::zero()),
            (Mode::Limb1, Mode::Limb1) => {
                let a = self.inner.as_limb1().expect("Limb1");
                let b = rhs.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                Ok(Self { inner: MagnitudePair::from_u128(limb_kernel::mul_1x1(a, b)) })
            }
            (Mode::Limb1, Mode::Limb2) => {
                let a = self.inner.as_limb1().expect("Limb1");
                let b = rhs.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::mul_2x1(b, a);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb1) => {
                let a = self.inner.as_limb2().expect("Limb2");
                let b = rhs.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(3)?;
                let (limbs, len) = limb_kernel::mul_2x1(a, b);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            (Mode::Limb2, Mode::Limb2) => {
                let a = self.inner.as_limb2().expect("Limb2");
                let b = rhs.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(4)?;
                let (limbs, len) = limb_kernel::mul_2(a, b);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            _ => Ok(Self::from_limbs(limb_kernel::mul_budgeted(
                self.as_limbs(),
                rhs.as_limbs(),
                ctx.budget(),
            )?)),
        }
    }

    /// 平方（默认 [`NumericContext::pure_rust_default`]）。
    pub fn sqr(&self) -> Self {
        self.try_sqr(&NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 平方（服从 `ctx` 预算）。
    pub fn try_sqr(&self, ctx: &NumericContext) -> Result<Self> {
        match self.inner.mode() {
            Mode::Zero => Ok(Self::zero()),
            Mode::Limb1 => {
                let a = self.inner.as_limb1().expect("Limb1");
                ctx.budget().check_limbs(2)?;
                Ok(Self { inner: MagnitudePair::from_u128(limb_kernel::mul_1x1(a, a)) })
            }
            Mode::Limb2 => {
                let a = self.inner.as_limb2().expect("Limb2");
                ctx.budget().check_limbs(4)?;
                let (limbs, len) = limb_kernel::mul_2(a, a);
                Ok(Self::from_fixed(&limbs[..len]))
            }
            Mode::Heap => Ok(Self::from_limbs(limb_kernel::sqr_budgeted(self.as_limbs(), ctx.budget())?)),
        }
    }

    /// 除法与余数（`rhs > 0`；默认上下文）。
    pub fn div_rem(&self, rhs: &Self) -> (Self, Self) {
        self.try_div_rem(rhs, &NumericContext::pure_rust_default())
            .expect("div_rem divisor non-zero and unbounded default")
    }

    /// 除法与余数（服从 `ctx` 预算；除数为零返回诊断）。
    pub fn try_div_rem(&self, rhs: &Self, ctx: &NumericContext) -> Result<(Self, Self)> {
        if rhs.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "natural_div_rem"));
        }
        let (q, r) = limb_kernel::div_rem_budgeted(self.as_limbs(), rhs.as_limbs(), ctx.budget())?;
        Ok((Self::from_limbs(q), Self::from_limbs(r)))
    }

    /// 模幂（`modulus > 0`；默认上下文）。
    pub fn mod_pow(&self, exp: &Self, modulus: &Self) -> Self {
        self.try_mod_pow(exp, modulus, &NumericContext::pure_rust_default())
            .expect("mod_pow modulus non-zero and unbounded default")
    }

    /// 模幂（服从 `ctx` 预算；奇模数足够宽时走 Montgomery）。
    pub fn try_mod_pow(&self, exp: &Self, modulus: &Self, ctx: &NumericContext) -> Result<Self> {
        if modulus.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::ModulusInvalid)
                .detail("domain", "numeric")
                .detail("operation", "natural_mod_pow")
                .detail("reason", "modulus_zero"));
        }
        ctx.budget().check_limbs(modulus.as_limbs().len())?;
        if modulus.is_one() {
            return Ok(Self::zero());
        }
        if limb_kernel::mod_pow_montgomery_eligible(modulus.as_limbs()) {
            return Ok(Self::from_limbs(limb_kernel::mod_pow_montgomery(
                self.as_limbs(),
                exp.as_limbs(),
                modulus.as_limbs(),
            )));
        }
        let mut result = Self::one();
        let mut base = self.try_div_rem(modulus, ctx)?.1;
        let mut e = exp.clone();
        while !e.is_zero() {
            if e.is_odd() {
                result = result.try_mul(&base, ctx)?.try_div_rem(modulus, ctx)?.1;
            }
            base = base.try_sqr(ctx)?.try_div_rem(modulus, ctx)?.1;
            e.div2();
        }
        Ok(result)
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        if self.is_zero() {
            return false;
        }
        let mut ones = 0u32;
        for &limb in self.as_limbs() {
            ones += limb.count_ones();
            if ones > 1 {
                return false;
            }
        }
        ones == 1
    }

    /// 十进制字符串（无符号）。
    pub fn to_decimal_string(&self) -> String {
        if self.is_zero() {
            return "0".to_string();
        }
        let (mut q, mut r) = self.clone().div_rem(&Self::from_u64(10));
        let mut digits = vec![b'0' + r.as_limbs()[0] as u8];
        while !q.is_zero() {
            (q, r) = q.div_rem(&Self::from_u64(10));
            digits.push(b'0' + r.as_limbs()[0] as u8);
        }
        digits.reverse();
        String::from_utf8(digits).unwrap_or_else(|_| "0".to_string())
    }

    /// 可落入 `u64` 时返回（零 → `Some(0)`）。
    pub fn to_u64(&self) -> Option<u64> {
        if self.is_zero() {
            return Some(0);
        }
        let limbs = self.as_limbs();
        if limbs.len() == 1 {
            Some(limbs[0])
        } else {
            None
        }
    }

    /// 可落入 `u128` 时返回。
    pub fn to_u128(&self) -> Option<u128> {
        if self.is_zero() {
            return Some(0);
        }
        let limbs = self.as_limbs();
        match limbs.len() {
            1 => Some(limbs[0] as u128),
            2 => Some(limbs[0] as u128 | ((limbs[1] as u128) << 64)),
            _ => None,
        }
    }

    /// 非负最大公约数（默认 [`NumericContext::pure_rust_default`]）。
    pub fn gcd(&self, other: &Self) -> Self {
        self.try_gcd(other, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 非负最大公约数（服从 `ctx` 预算；结果 limb ≤ `min(self, other)`）。
    pub fn try_gcd(&self, other: &Self, ctx: &NumericContext) -> Result<Self> {
        if self.is_zero() && other.is_zero() {
            return Ok(Self::zero());
        }
        let bound = self.as_limbs().len().min(other.as_limbs().len()).max(1);
        ctx.budget().check_limbs(bound)?;
        Ok(Self::from_limbs(limb_kernel::gcd(self.as_limbs().to_vec(), other.as_limbs().to_vec())))
    }

    /// 借用小端 limb（生命周期绑在 `&self`；禁止跨 move 持有）。
    pub fn as_limbs(&self) -> &[u64] {
        self.inner.as_limbs()
    }

    /// 由小端 limb 构造（已规范化）。
    pub fn from_limbs(limbs: Vec<u64>) -> Self {
        let n = Self { inner: MagnitudePair::from_limbs(&limbs) };
        #[cfg(debug_assertions)]
        n.debug_assert_invariants();
        n
    }

    /// 由无符号 [`MagnitudePair`] 构造（清除 sign 位）。
    pub(crate) fn from_pair(inner: MagnitudePair) -> Self {
        let n = Self { inner: inner.with_negative(false) };
        #[cfg(debug_assertions)]
        n.debug_assert_invariants();
        n
    }

    /// 取出内部 tagged 表示。
    pub(crate) fn into_pair(self) -> MagnitudePair {
        self.inner
    }

    /// 固定宽度结果回写（≤4 limbs，不经中间 `Vec`）。
    #[inline]
    fn from_fixed(limbs: &[u64]) -> Self {
        let n = Self { inner: MagnitudePair::from_fixed_limbs(limbs) };
        #[cfg(debug_assertions)]
        n.debug_assert_invariants();
        n
    }

    /// 二进制 wire 幅度：`u32` 小端 limb 计数 + `u64` 小端 limb。
    ///
    /// 零编码为 `count=1` 且 limb `0`（与历史 `Vec` 表示一致）；解码亦接受 `count=0`。
    pub(crate) fn wire_encode_magnitude(&self) -> Vec<u8> {
        let limbs = self.as_limbs();
        let el = limbs.len();
        let mut out = Vec::with_capacity(4 + el * 8);
        out.extend_from_slice(&(el as u32).to_le_bytes());
        for &limb in limbs {
            out.extend_from_slice(&limb.to_le_bytes());
        }
        out
    }

    /// 在执行预算下解码 [`Self::wire_encode_magnitude`] 字节。
    pub(crate) fn wire_decode_magnitude_budgeted(
        bytes: &[u8],
        budget: &crate::policy::execution_budget::ExecutionBudget,
    ) -> std::result::Result<Self, ()> {
        if bytes.len() < 4 {
            return Err(());
        }
        budget.check_wire_bytes(bytes.len()).map_err(|_| ())?;
        let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| ())?) as usize;
        budget.check_limbs(count).map_err(|_| ())?;
        let need = 4usize.checked_add(count.checked_mul(8).ok_or(())?).ok_or(())?;
        if bytes.len() != need {
            return Err(());
        }
        if count == 0 {
            return Ok(Self::zero());
        }
        let mut limbs = Vec::with_capacity(count);
        for i in 0..count {
            let off = 4 + i * 8;
            limbs.push(u64::from_le_bytes(bytes[off..off + 8].try_into().map_err(|_| ())?));
        }
        Ok(Self::from_limbs(limbs))
    }

    /// 解码 [`Self::wire_encode_magnitude`] 字节。
    pub(crate) fn wire_decode_magnitude(bytes: &[u8]) -> std::result::Result<Self, ()> {
        Self::wire_decode_magnitude_budgeted(bytes, &crate::policy::execution_budget::ExecutionBudget::unlimited())
    }

    /// 从拼接的有理载荷中拆出首个幅度块。
    pub(crate) fn wire_take_magnitude(bytes: &[u8]) -> std::result::Result<(Self, &[u8]), ()> {
        if bytes.len() < 4 {
            return Err(());
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().map_err(|_| ())?) as usize;
        let total = 4usize.checked_add(count.checked_mul(8).ok_or(())?).ok_or(())?;
        if bytes.len() < total {
            return Err(());
        }
        let mag = Self::wire_decode_magnitude(&bytes[..total])?;
        Ok((mag, &bytes[total..]))
    }

    #[cfg(debug_assertions)]
    fn debug_assert_invariants(&self) {
        let limbs = self.as_limbs();
        debug_assert!(!limbs.is_empty(), "Natural as_limbs must expose at least [0]");
        if self.is_zero() {
            debug_assert_eq!(limbs, &[0]);
            debug_assert!(matches!(self.inner.mode(), Mode::Zero));
        } else {
            debug_assert_ne!(*limbs.last().unwrap(), 0);
            match limbs.len() {
                1 => debug_assert!(matches!(self.inner.mode(), Mode::Limb1)),
                2 => debug_assert!(matches!(self.inner.mode(), Mode::Limb2)),
                n => {
                    debug_assert!(n >= 3);
                    debug_assert!(matches!(self.inner.mode(), Mode::Heap));
                }
            }
        }
    }
}

impl FromStr for Natural {
    type Err = ();

    /// 十进制解析（仅数字，无符号）。
    fn from_str(digits: &str) -> std::result::Result<Self, Self::Err> {
        if digits.is_empty() {
            return Err(());
        }
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return Err(());
        }
        let mut n = Self::zero();
        for ch in digits.chars() {
            n = n.mul_u64(10).add_u64(u64::from(ch as u32 - u32::from(b'0')));
        }
        Ok(n)
    }
}
