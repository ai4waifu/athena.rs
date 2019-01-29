//! 精确有理（分子 / 分母各自 `meta + Magnitude`；分母 unsigned / nonzero）。

use athena_types::{Diagnostic, DiagnosticCode};
use std::cmp::Ordering;

use crate::{
    format::validation::{reject_non_canonical, WireReject},
    magnitude::MagnitudePair,
    policy::execution_budget::NumericContext,
    value::integer::{Integer, Sign},
    value::natural::Natural,
};

/// 精确有理（既约、分母为正；亦称 [`ExactRational`]）。
///
/// 布局：`numerator_meta + Magnitude` + `denominator_meta + Magnitude`（LP64 上 48 bytes）。
/// 分母恒为 unsigned 非零；零有理规范为分子零且分母一。
///
/// 不实现 [`Ord`]：域上字典序不是数值序。请用 [`Self::cmp_numeric`]。
#[derive(Clone)]
pub struct Rational {
    numer: MagnitudePair,
    denom: MagnitudePair,
}

/// 稳定别名（与 [`NumericValue`] 同义迁移期命名）。
pub type ExactRational = Rational;

#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(core::mem::size_of::<Rational>() == 48);
    assert!(core::mem::align_of::<Rational>() == 8);
};

impl core::fmt::Debug for Rational {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Rational")
            .field("numer", &self.numerator())
            .field("denom", &self.denominator())
            .finish()
    }
}

impl PartialEq for Rational {
    fn eq(&self, other: &Self) -> bool {
        self.numerator() == other.numerator() && self.denominator() == other.denominator()
    }
}

impl Eq for Rational {}

impl Rational {
    fn from_parts(numer: Integer, denom: Natural) -> Self {
        debug_assert!(!denom.is_zero());
        Self {
            numer: numer.into_pair(),
            denom: denom.into_pair(),
        }
    }

    /// 由整数构造。
    pub fn from_integer(n: Integer) -> Self {
        Self::from_parts(n, Natural::one())
    }

    /// 分子 / 分母（自动约分；分母为零失败）。
    pub fn try_new(numer: Integer, denom: Integer) -> Result<Self, Diagnostic> {
        if denom.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_new"));
        }
        Ok(Self::normalize_pair(numer, denom))
    }

    /// 分子 / 分母（自动约分）。分母为零时 panic — 优先用 [`try_new`]。
    pub fn new(numer: Integer, denom: Integer) -> Self {
        Self::try_new(numer, denom).expect("rational denominator must be non-zero")
    }

    /// ANV1：仅接受已既约且分母为正的规范对（拒绝静默约分）。
    pub(crate) fn try_from_canonical_wire(numer: Integer, denom: Integer) -> Result<Self, Diagnostic> {
        if denom.is_zero() {
            return Err(reject_non_canonical(WireReject::RationalDenomZero));
        }
        if denom.is_negative() {
            return Err(reject_non_canonical(WireReject::RationalDenomSign));
        }
        if numer.is_zero() {
            if !denom.is_one() {
                return Err(reject_non_canonical(WireReject::RationalZeroDenomNotOne));
            }
            return Ok(Self::zero());
        }
        let g = numer.abs().gcd(&denom.abs());
        if !g.is_one() {
            return Err(reject_non_canonical(WireReject::RationalUnreduced));
        }
        Ok(Self::from_parts(numer, denom.magnitude()))
    }

    fn normalize_pair(numer: Integer, denom: Integer) -> Self {
        if denom.is_zero() {
            return Self::zero();
        }
        let g = numer.abs().gcd(&denom.abs());
        let mut n = if g.is_one() { numer } else { numer.div(&g).expect("gcd") };
        let mut d = if g.is_one() { denom } else { denom.div(&g).expect("gcd") };
        if d.is_negative() {
            n = n.neg();
            d = d.neg();
        }
        if n.is_zero() {
            return Self::zero();
        }
        let denom_nat = if d.is_one() {
            Natural::one()
        } else {
            d.magnitude()
        };
        Self::from_parts(n, denom_nat)
    }

    /// 显式规范化（正分母、既约）。
    pub fn normalize(self) -> Self {
        Self::normalize_pair(self.numerator(), self.denominator())
    }

    /// 零。
    pub fn zero() -> Self {
        Self::from_parts(Integer::zero(), Natural::one())
    }

    /// 一。
    pub fn one() -> Self {
        Self::from_parts(Integer::one(), Natural::one())
    }

    /// 分子。
    pub fn numerator(&self) -> Integer {
        Integer::from_pair(self.numer.clone())
    }

    /// 分母（恒为正；整数 / 零时为 1）。
    pub fn denominator(&self) -> Integer {
        if self.is_zero() {
            Integer::one()
        } else {
            Integer::from_positive_natural(Natural::from_pair(self.denom.clone()))
        }
    }

    /// 是否为零。
    pub fn is_zero(&self) -> bool {
        self.numer.is_zero()
    }

    /// 是否为负。
    pub fn is_negative(&self) -> bool {
        self.numer.is_negative()
    }

    /// 是否非负（零计为非负）。
    pub fn is_non_negative(&self) -> bool {
        !self.is_negative()
    }

    /// 是否为整数（分母为 1）。
    pub fn is_integer(&self) -> bool {
        Natural::from_pair(self.denom.clone()).is_one()
    }

    /// 符号。
    pub fn sign(&self) -> Sign {
        if self.numer.is_zero() {
            Sign::Zero
        } else if self.numer.is_negative() {
            Sign::Negative
        } else {
            Sign::Positive
        }
    }

    /// 数值比较：先交叉约分再比较 `a*d` 与 `c*b`。
    pub fn cmp_numeric(&self, other: &Self) -> Ordering {
        if self == other {
            return Ordering::Equal;
        }
        let mut a = self.numerator();
        let mut b = self.denominator();
        let mut c = other.numerator();
        let mut d = other.denominator();
        let g1 = a.abs().gcd(&c.abs());
        if !g1.is_one() {
            a = a.div(&g1).expect("gcd");
            c = c.div(&g1).expect("gcd");
        }
        let g2 = b.abs().gcd(&d.abs());
        if !g2.is_one() {
            b = b.div(&g2).expect("gcd");
            d = d.div(&g2).expect("gcd");
        }
        a.mul(&d).cmp(&c.mul(&b))
    }

    /// 绝对值。
    pub fn abs(&self) -> Self {
        Self::from_parts(self.numerator().abs(), Natural::from_pair(self.denom.clone()))
    }

    /// 取负。
    pub fn neg(&self) -> Self {
        Self::from_parts(self.numerator().neg(), Natural::from_pair(self.denom.clone()))
    }

    /// 加法（合并前交叉约去 `gcd(b,d)`；默认上下文）。
    pub fn add(&self, rhs: &Self) -> Self {
        self.try_add(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 加法（服从 `ctx` 预算）。
    pub fn try_add(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self, Diagnostic> {
        let mut b = self.denominator();
        let mut d = rhs.denominator();
        let g = b.abs().try_gcd(&d.abs(), ctx)?;
        if !g.is_one() {
            b = b.try_div_rem_trunc(&g, ctx)?.0;
            d = d.try_div_rem_trunc(&g, ctx)?.0;
        }
        let n = self
            .numerator()
            .try_mul(&d, ctx)?
            .try_add(&rhs.numerator().try_mul(&b, ctx)?, ctx)?;
        let denom = self.denominator().try_mul(&d, ctx)?;
        Ok(Self::normalize_pair(n, denom))
    }

    /// 减法（默认上下文）。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.add(&rhs.neg())
    }

    /// 减法（服从 `ctx` 预算）。
    pub fn try_sub(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self, Diagnostic> {
        self.try_add(&rhs.neg(), ctx)
    }

    /// 乘法（乘积前交叉约分；默认上下文）。
    pub fn mul(&self, rhs: &Self) -> Self {
        self.try_mul(rhs, &NumericContext::pure_rust_default())
            .expect("pure-rust default max_limbs unbounded")
    }

    /// 乘法（服从 `ctx` 预算）。
    pub fn try_mul(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self, Diagnostic> {
        let (n, d) = cross_cancel_mul_ctx(
            self.numerator(),
            self.denominator(),
            rhs.numerator(),
            rhs.denominator(),
            ctx,
        )?;
        Ok(Self::normalize_pair(n, d))
    }

    /// 除法（交叉约分后做 `a/b * d/c`；默认上下文）。
    pub fn try_div(&self, rhs: &Self) -> Result<Self, Diagnostic> {
        self.try_div_ctx(rhs, &NumericContext::pure_rust_default())
    }

    /// 除法（服从 `ctx` 预算）。
    pub fn try_div_ctx(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self, Diagnostic> {
        if rhs.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::DivideByZero)
                .detail("domain", "numeric")
                .detail("operation", "rational_div"));
        }
        let (n, d) = cross_cancel_mul_ctx(
            self.numerator(),
            self.denominator(),
            rhs.denominator(),
            rhs.numerator(),
            ctx,
        )?;
        Ok(Self::normalize_pair(n, d))
    }

    /// 非负整数幂。
    pub fn pow_u32(&self, exp: u32) -> Result<Self, Diagnostic> {
        if exp == 0 {
            return Ok(Self::one());
        }
        let n = self
            .numerator()
            .pow_u32(exp)
            .map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
        let d = self
            .denominator()
            .pow_u32(exp)
            .map_err(|_| Diagnostic::new(DiagnosticCode::ExponentOutOfRange))?;
        Ok(Self::normalize_pair(n, d))
    }

    /// 可完全表示时精确转为 binary64。
    pub fn try_to_f64_exact(&self) -> Option<f64> {
        if self.is_integer() {
            return self.numerator().try_to_f64_exact();
        }
        let d = self.denominator();
        if !d.is_power_of_two() {
            return None;
        }
        let nf = self.numerator().try_to_f64_exact()?;
        let df = d.try_to_f64_exact()?;
        if df == 0.0 {
            return None;
        }
        let q = nf / df;
        if !q.is_finite() {
            return None;
        }
        if nf.to_bits() == (q * df).to_bits() {
            Some(q)
        } else {
            None
        }
    }

    /// 显式近似 `f64`。
    pub fn to_f64_approximate(&self) -> Option<f64> {
        let nf = self.numerator().to_f64_approximate()?;
        let df = self.denominator().to_f64_approximate()?;
        if df == 0.0 {
            return None;
        }
        let q = nf / df;
        if q.is_finite() {
            Some(q)
        } else {
            None
        }
    }

    /// [`try_to_f64_exact`] 的别名。
    pub fn to_f64_exact_machine(&self) -> Option<f64> {
        self.try_to_f64_exact()
    }

    /// 供宿主文本渲染的 `numer/denom` 十进制载荷。
    pub fn to_wire_string(&self) -> String {
        if self.is_integer() {
            self.numerator().to_decimal_string()
        } else {
            format!(
                "{}/{}",
                self.numerator().to_decimal_string(),
                self.denominator().to_decimal_string()
            )
        }
    }
}

/// 相乘 `a/b * c/d` 前交叉约分（服从预算）。
fn cross_cancel_mul_ctx(
    a: Integer,
    b: Integer,
    c: Integer,
    d: Integer,
    ctx: &NumericContext,
) -> Result<(Integer, Integer), Diagnostic> {
    let mut a = a;
    let mut b = b;
    let mut c = c;
    let mut d = d;
    let g1 = a.abs().try_gcd(&d.abs(), ctx)?;
    if !g1.is_one() {
        a = a.try_div_rem_trunc(&g1, ctx)?.0;
        d = d.try_div_rem_trunc(&g1, ctx)?.0;
    }
    let g2 = c.abs().try_gcd(&b.abs(), ctx)?;
    if !g2.is_one() {
        c = c.try_div_rem_trunc(&g2, ctx)?.0;
        b = b.try_div_rem_trunc(&g2, ctx)?.0;
    }
    Ok((a.try_mul(&c, ctx)?, b.try_mul(&d, ctx)?))
}
