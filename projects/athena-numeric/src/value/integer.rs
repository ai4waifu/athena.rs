//! 精确整数（自有 `meta + Magnitude`；`Sign` 仅为语义 API）。

use athena_types::{Diagnostic, DiagnosticCode, Result};
use std::{cmp::Ordering, str::FromStr};

use crate::{
    dispatch::NumericExecutor,
    execution_budget::NumericContext,
    kernel::limb as limb_kernel,
    natural::Natural,
    storage::{MagnitudePair, gc_alloc_error},
};

/// 只读幅度视图：借用 limb + 符号（Living 18）。
///
/// 生命周期不得长过借出的 [`Integer`]。算术热路径应经本视图进入算法，
/// 禁止为读取符号 / 绝对值而 owning clone Heap magnitude。
#[derive(Debug, Clone, Copy)]
pub struct MagnitudeView<'a> {
    limbs: &'a [u64],
    negative: bool,
}

impl<'a> MagnitudeView<'a> {
    /// 由已存在的 limb 切片与符号构造。
    #[inline]
    pub fn from_parts(limbs: &'a [u64], negative: bool) -> Self {
        let zero = limbs.is_empty() || limb_kernel::is_zero(limbs);
        Self { limbs, negative: negative && !zero }
    }

    /// 小端 limb（生命周期绑在借出方）。
    #[inline]
    pub fn limbs(self) -> &'a [u64] {
        self.limbs
    }

    /// 是否为负（零恒为 false）。
    #[inline]
    pub fn is_negative(self) -> bool {
        self.negative && !self.is_zero()
    }

    /// 是否为零（`[0]` 与空切片）。
    #[inline]
    pub fn is_zero(self) -> bool {
        self.limbs.is_empty() || limb_kernel::is_zero(self.limbs)
    }

    /// 符号。
    #[inline]
    pub fn sign(self) -> Sign {
        if self.is_zero() {
            Sign::Zero
        }
        else if self.negative {
            Sign::Negative
        }
        else {
            Sign::Positive
        }
    }
}

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
/// 经私有 [`MagnitudePair`] 做 Drop；无独立 `Sign` 字段、不嵌套 `Natural`。
/// 排序必须是数学序：负数额值反序、正数额值正序。禁止 derive `Ord`。
///
/// # 复制合同（Living `19`）
///
/// **不**实现 [`Clone`]。Limb1/Limb2 用 [`Self::clone_inline`]；Heap owning 深复制用
/// [`Self::try_clone_in`]。算术热路径经 [`MagnitudeView`] 借用，结果经 context 发布。
#[derive(Debug, PartialEq, Eq, Hash)]
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

/// 精确整数的稳定公开名（与 [`Integer`] 同义）。
pub type ExactInteger = Integer;

impl Integer {
    /// 由幅度与符号构造（不复制 limb；幅度所有权转入本值）。
    pub fn from_natural_sign(mag: Natural, negative: bool) -> Self {
        Self::from_mag_sign(mag, negative)
    }

    fn from_mag_sign(mag: Natural, negative: bool) -> Self {
        Self { inner: mag.into_pair().with_negative(negative) }
    }

    pub(crate) fn from_pair(inner: MagnitudePair) -> Self {
        let i = Self { inner };
        #[cfg(debug_assertions)]
        i.debug_assert_invariants();
        i
    }

    pub(crate) fn into_pair(self) -> MagnitudePair {
        self.inner
    }

    #[cfg(debug_assertions)]
    fn debug_assert_invariants(&self) {
        if matches!(self.inner.mode(), crate::storage::Mode::Heap) {
            debug_assert!(self.inner.is_heap_rooted(), "Integer Heap must be rooted PublishedNumericBlock");
        }
    }

    /// 无符号幅度（可失败 owning 复制；仅供确需 `Natural` 所有权的路径）。
    ///
    /// Living `19`/`24`：算术热路径请用 [`Self::magnitude_view`] / [`Self::as_limbs`]。
    fn try_abs_natural(&self) -> athena_gc::Result<Natural> {
        Ok(Natural::from_pair(self.inner.try_clone_clear_sign()?))
    }

    /// 借用小端幅度 limb（生命周期绑在 `&self`）。
    #[inline]
    pub fn as_limbs(&self) -> &[u64] {
        self.inner.as_limbs()
    }

    /// 只读幅度视图（不 clone）。
    #[inline]
    pub fn magnitude_view(&self) -> MagnitudeView<'_> {
        MagnitudeView::from_parts(self.as_limbs(), self.inner.is_negative())
    }

    /// 可失败 owning 复制（Heap 经目标 `ctx` 发布为 `PublishedNumericBlock`）。
    pub fn try_clone_in(&self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(Self::from_pair(self.inner.try_clone_on(ctx.heap()).map_err(gc_alloc_error)?))
    }

    /// Limb1 / Limb2 栈拷贝；Heap 返回 `None`（Living `19`）。
    #[inline]
    pub fn clone_inline(&self) -> Option<Self> {
        Some(Self::from_pair(self.inner.clone_inline()?))
    }

    /// 将借用幅度按符号发布到 `ctx` heap（结果 owning）。
    fn publish_signed(ctx: &NumericContext, limbs: &[u64], negative: bool) -> Result<Self> {
        Ok(Self::from_mag_sign(Natural::from_limb_slice_in(ctx, limbs)?, negative))
    }

    /// 在 `ctx` heap 上由小端 limb 构造非负整数（session / numeric 发布）。
    ///
    /// 与 [`Natural::from_limbs_in`] 对齐。无 `ctx` 的便利入口走 [`NumericContext::portable_default`]。
    pub fn from_limbs_in(ctx: &NumericContext, limbs: impl AsRef<[u64]>) -> Result<Self> {
        Self::publish_signed(ctx, limbs.as_ref(), false)
    }

    /// 由已解码 `i64` 构造。
    pub fn from_i64(n: i64) -> Self {
        if n == 0 {
            Self::zero()
        }
        else if n < 0 {
            Self::from_mag_sign(Natural::from_u64(n.unsigned_abs()), true)
        }
        else {
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
        self.try_abs_natural().expect("portable default max_limbs unbounded")
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
        }
        else if self.inner.is_negative() {
            Sign::Negative
        }
        else {
            Sign::Positive
        }
    }

    /// 绝对值（无 `ctx` 便利入口；Heap 经同堆深复制，与 [`Self::add`] 同合同）。
    pub fn abs(&self) -> Self {
        self.try_abs(&NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 绝对值（服从 `ctx` 预算）。
    pub fn try_abs(&self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(Self::from_pair(self.inner.try_clone_clear_sign().map_err(gc_alloc_error)?))
    }

    /// 取负（无 `ctx` 便利入口）。
    pub fn neg(&self) -> Self {
        self.try_neg(&NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 取负（服从 `ctx` 预算；不经 owning `Clone`）。
    pub fn try_neg(&self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(match self.sign() {
            Sign::Zero => Self::zero(),
            Sign::Positive => {
                let p = self.inner.try_clone().map_err(gc_alloc_error)?;
                Ok::<_, Diagnostic>(Self::from_pair(p.with_negative(true)))?
            }
            Sign::Negative => Self::from_pair(self.inner.try_clone_clear_sign().map_err(gc_alloc_error)?),
        })
    }

    /// 非负最大公约数；`gcd(0,0) = 0`（默认上下文）。
    pub fn gcd(&self, other: &Self) -> Self {
        self.try_gcd(other, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 非负最大公约数（服从 `ctx` 预算）。
    pub fn try_gcd(&self, other: &Self, ctx: &NumericContext) -> Result<Self> {
        Self::try_gcd_view(self.magnitude_view(), other.magnitude_view(), ctx)
    }

    /// 借用视图最大公约数；结果发布到 `ctx`。
    pub fn try_gcd_view(lhs: MagnitudeView<'_>, rhs: MagnitudeView<'_>, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if lhs.is_zero() && rhs.is_zero() {
            return Ok(Self::zero());
        }
        let g = Natural::try_gcd_limbs(lhs.limbs(), rhs.limbs(), ctx)?;
        Ok(Self::from_positive_natural(g))
    }

    /// 加法（默认 [`NumericContext::portable_default`]）。
    pub fn add(&self, rhs: &Self) -> Self {
        self.try_add(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 加法（服从 `ctx` 预算）。
    pub fn try_add(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        Self::try_add_view(self.magnitude_view(), rhs.magnitude_view(), ctx)
    }

    /// 消费 `self` 的加法：同号时经 [`Natural::try_add_owned`] 就地复用幅度缓冲；异号走减幅度。
    ///
    /// 不经 owning 幅度复制热路径；`rhs` 幅度仅在需要时做 clear-sign clone 以进入 `Natural` API。
    pub fn try_add_owned(self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if rhs.is_zero() {
            return Ok(self);
        }
        if self.is_zero() {
            return rhs.try_clone_in(ctx);
        }

        let lhs_neg = self.is_negative();
        let rhs_neg = rhs.is_negative();
        if lhs_neg == rhs_neg {
            let mag = Natural::from_pair(self.into_pair().with_negative(false));
            let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
            return Ok(Self::from_mag_sign(mag.try_add_owned(&rhs_mag, ctx)?, lhs_neg));
        }

        // Opposite signs: |a| - |b| with sign of the larger magnitude.
        let cmp = limb_kernel::cmp_slice(self.as_limbs(), rhs.as_limbs());
        match cmp {
            Ordering::Equal => Ok(Self::zero()),
            Ordering::Greater => {
                let mag = Natural::from_pair(self.into_pair().with_negative(false));
                let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
                Ok(Self::from_mag_sign(mag.try_sub_owned(&rhs_mag, ctx)?, lhs_neg))
            }
            Ordering::Less => {
                // Result takes rhs sign; cannot steal self as destination (smaller).
                let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
                let lhs_mag = Natural::from_pair(self.into_pair().with_negative(false));
                Ok(Self::from_mag_sign(rhs_mag.try_sub_owned(&lhs_mag, ctx)?, rhs_neg))
            }
        }
    }

    /// 借用视图加法；结果发布到 `ctx`（Living 18）。
    pub fn try_add_view(lhs: MagnitudeView<'_>, rhs: MagnitudeView<'_>, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        Ok(match (lhs.sign(), rhs.sign()) {
            (Sign::Zero, _) => Self::publish_signed(ctx, rhs.limbs(), rhs.is_negative())?,
            (_, Sign::Zero) => Self::publish_signed(ctx, lhs.limbs(), lhs.is_negative())?,
            (Sign::Positive, Sign::Positive) => Self::from_positive_natural(NumericExecutor::add_limbs(lhs.limbs(), rhs.limbs(), ctx)?),
            (Sign::Negative, Sign::Negative) => Self::from_mag_sign(NumericExecutor::add_limbs(lhs.limbs(), rhs.limbs(), ctx)?, true),
            (Sign::Positive, Sign::Negative) | (Sign::Negative, Sign::Positive) => match limb_kernel::cmp_slice(lhs.limbs(), rhs.limbs()) {
                Ordering::Greater | Ordering::Equal => {
                    let mag = NumericExecutor::sub_limbs(lhs.limbs(), rhs.limbs(), ctx)?;
                    Self::from_mag_sign(mag, lhs.is_negative())
                }
                Ordering::Less => {
                    let mag = NumericExecutor::sub_limbs(rhs.limbs(), lhs.limbs(), ctx)?;
                    Self::from_mag_sign(mag, rhs.is_negative())
                }
            },
        })
    }

    /// 减法（默认上下文）。
    pub fn sub(&self, rhs: &Self) -> Self {
        self.try_sub(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 减法（服从 `ctx` 预算；不经 owning `neg`）。
    pub fn try_sub(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        Self::try_sub_view(self.magnitude_view(), rhs.magnitude_view(), ctx)
    }

    /// 消费 `self` 的减法：与 [`Self::try_add_owned`] 同路径，仅将 `rhs` 符号取反。
    ///
    /// 不经 owning [`Self::neg`]；幅度 clone 仅在进入 `Natural` owned API 时发生。
    pub fn try_sub_owned(self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if rhs.is_zero() {
            return Ok(self);
        }
        if self.is_zero() {
            // 0 - b = -b（幅度 clear-sign clone；符号取反）。
            return Ok(Self::from_mag_sign(Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?), !rhs.is_negative()));
        }

        let lhs_neg = self.is_negative();
        let rhs_neg = !rhs.is_negative(); // flipped relative to try_add_owned
        if lhs_neg == rhs_neg {
            let mag = Natural::from_pair(self.into_pair().with_negative(false));
            let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
            return Ok(Self::from_mag_sign(mag.try_add_owned(&rhs_mag, ctx)?, lhs_neg));
        }

        let cmp = limb_kernel::cmp_slice(self.as_limbs(), rhs.as_limbs());
        match cmp {
            Ordering::Equal => Ok(Self::zero()),
            Ordering::Greater => {
                let mag = Natural::from_pair(self.into_pair().with_negative(false));
                let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
                Ok(Self::from_mag_sign(mag.try_sub_owned(&rhs_mag, ctx)?, lhs_neg))
            }
            Ordering::Less => {
                let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
                let lhs_mag = Natural::from_pair(self.into_pair().with_negative(false));
                Ok(Self::from_mag_sign(rhs_mag.try_sub_owned(&lhs_mag, ctx)?, rhs_neg))
            }
        }
    }

    /// 借用视图减法；结果发布到 `ctx`。
    pub fn try_sub_view(lhs: MagnitudeView<'_>, rhs: MagnitudeView<'_>, ctx: &NumericContext) -> Result<Self> {
        let rhs_neg = MagnitudeView::from_parts(rhs.limbs(), !rhs.is_negative());
        Self::try_add_view(lhs, rhs_neg, ctx)
    }

    /// 乘法（默认上下文）。
    pub fn mul(&self, rhs: &Self) -> Self {
        self.try_mul(rhs, &NumericContext::portable_default()).expect("portable default max_limbs unbounded")
    }

    /// 乘法（服从 `ctx` 预算）。
    pub fn try_mul(&self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        Self::try_mul_view(self.magnitude_view(), rhs.magnitude_view(), ctx)
    }

    /// 消费 `self` 的乘法：经 [`Natural::try_mul_owned`]（Schoolbook + 余量容量时 steal）。
    pub fn try_mul_owned(self, rhs: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if self.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let neg = self.is_negative() != rhs.is_negative();
        let mag = Natural::from_pair(self.into_pair().with_negative(false));
        let rhs_mag = Natural::from_pair(rhs.inner.try_clone_clear_sign().map_err(gc_alloc_error)?);
        Ok(Self::from_mag_sign(mag.try_mul_owned(&rhs_mag, ctx)?, neg))
    }

    /// 消费 `self` 的 `× u64`：经 [`Natural::try_mul_u64_owned`] 就地复用幅度。
    pub fn try_mul_u64_owned(self, rhs: u64, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if self.is_zero() || rhs == 0 {
            return Ok(Self::zero());
        }
        let neg = self.is_negative();
        let mag = Natural::from_pair(self.into_pair().with_negative(false));
        Ok(Self::from_mag_sign(mag.try_mul_u64_owned(rhs, ctx)?, neg))
    }

    /// 借用视图乘法；结果发布到 `ctx`。
    pub fn try_mul_view(lhs: MagnitudeView<'_>, rhs: MagnitudeView<'_>, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
        if lhs.is_zero() || rhs.is_zero() {
            return Ok(Self::zero());
        }
        let negative = lhs.is_negative() != rhs.is_negative();
        Ok(Self::from_mag_sign(NumericExecutor::mul_limbs(lhs.limbs(), rhs.limbs(), ctx)?, negative))
    }

    /// 向零整除：商向零，余数与被除数同号（默认上下文）。
    pub fn div_rem_trunc(&self, rhs: &Self) -> Result<(Self, Self)> {
        self.try_div_rem_trunc(rhs, &NumericContext::portable_default())
    }

    /// 向零整除（服从 `ctx` 预算）。
    pub fn try_div_rem_trunc(&self, rhs: &Self, ctx: &NumericContext) -> Result<(Self, Self)> {
        Self::try_div_rem_trunc_view(self.magnitude_view(), rhs.magnitude_view(), ctx)
    }

    /// 借用视图向零整除；结果发布到 `ctx`。
    pub fn try_div_rem_trunc_view(lhs: MagnitudeView<'_>, rhs: MagnitudeView<'_>, ctx: &NumericContext) -> Result<(Self, Self)> {
        ctx.check_entry()?;
        if rhs.is_zero() {
            return Err(division_by_zero("div_rem_trunc"));
        }
        let (q_mag, r_mag) = Natural::try_div_rem_limbs(lhs.limbs(), rhs.limbs(), ctx)?;
        let q_neg = lhs.is_negative() != rhs.is_negative();
        let r_neg = lhs.is_negative();
        Ok((Self::from_mag_sign(q_mag, q_neg), Self::from_mag_sign(r_mag, r_neg)))
    }

    /// Euclidean 整除：余数满足 `0 <= r < |rhs|`（默认上下文）。
    pub fn div_rem_euclid(&self, rhs: &Self) -> Result<(Self, Self)> {
        self.try_div_rem_euclid(rhs, &NumericContext::portable_default())
    }

    /// Euclidean 整除（服从 `ctx` 预算）。
    pub fn try_div_rem_euclid(&self, rhs: &Self, ctx: &NumericContext) -> Result<(Self, Self)> {
        ctx.check_entry()?;
        if rhs.is_zero() {
            return Err(division_by_zero("div_rem_euclid"));
        }
        let (mut q, mut r) = self.try_div_rem_trunc(rhs, ctx)?;
        if r.is_negative() {
            if rhs.is_positive() {
                r = r.try_add(rhs, ctx)?;
                q = q.try_sub(&Self::one(), ctx)?;
            }
            else {
                r = r.try_sub(rhs, ctx)?;
                q = q.try_add(&Self::one(), ctx)?;
            }
        }
        debug_assert!(!r.is_negative());
        debug_assert!(limb_kernel::cmp_slice(r.as_limbs(), rhs.as_limbs()).is_lt() || r.is_zero());
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
        self.try_mod_pow(exp, modulus, &NumericContext::portable_default())
    }

    /// 模幂（服从 `ctx` 预算）。
    pub fn try_mod_pow(&self, exp: &Self, modulus: &Self, ctx: &NumericContext) -> Result<Self> {
        ctx.check_entry()?;
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
        let result_mag = Natural::try_mod_pow_limbs(base.as_limbs(), exp.as_limbs(), modulus.as_limbs(), ctx)?;
        Ok(Self::from_positive_natural(result_mag))
    }

    /// 绝对值的二进制位宽（`0` → `0`）。
    pub fn bits(&self) -> u64 {
        if self.is_zero() {
            return 0;
        }
        let limbs = self.as_limbs();
        let top = limbs.len() - 1;
        (top as u64) * 64 + (64 - limbs[top].leading_zeros() as u64)
    }

    /// 可无损落入 `i64` 时返回（含 `i64::MIN` 与零）。
    pub fn to_i64(&self) -> Option<i64> {
        if self.is_zero() {
            return Some(0);
        }
        let u = limbs_to_u128(self.as_limbs())?;
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
            Sign::Positive => limbs_to_u64(self.as_limbs()),
            Sign::Negative | Sign::Zero => None,
        }
    }

    /// 可无损落入 `u128` 时返回（零 → `Some(0)`）。
    pub fn to_u128(&self) -> Option<u128> {
        if self.is_zero() {
            return Some(0);
        }
        match self.sign() {
            Sign::Positive => limbs_to_u128(self.as_limbs()),
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
        let u = limbs_to_u128(self.as_limbs())?;
        if u > Self::F64_EXACT_ABS_MAX {
            return None;
        }
        let f = if self.is_negative() { -(u as f64) } else { u as f64 };
        if !f.is_finite() {
            return None;
        }
        if f64_represents_integer(f, self) { Some(f) } else { None }
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
    ///
    /// Living `19`：只借 `as_limbs()`，不经 owning 幅度复制 / `Clone`。
    pub fn to_decimal_string(&self) -> String {
        match self.sign() {
            Sign::Zero => "0".to_string(),
            Sign::Positive => Natural::decimal_from_limbs(self.as_limbs()),
            Sign::Negative => format!("-{}", Natural::decimal_from_limbs(self.as_limbs())),
        }
    }

    /// 是否为 2 的幂（正整数）。
    pub fn is_power_of_two(&self) -> bool {
        if !self.is_positive() {
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

    /// 是否为奇数。
    pub fn is_odd(&self) -> bool {
        !self.is_zero() && (self.as_limbs()[0] & 1) == 1
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
        let mut base = self.try_clone_in(&NumericContext::portable_default()).map_err(|_| ())?;
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
        }
        else {
            return Err(());
        }
        let mut acc = Self::one();
        let ctx = NumericContext::portable_default();
        let mut base = self.try_clone_in(&ctx).expect("portable default max_limbs unbounded");
        let mut e = exp.try_clone_in(&ctx).expect("portable default max_limbs unbounded");
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
        let mut hi = self.try_clone_in(&NumericContext::portable_default())?.add(&Self::one());
        let two = Integer::from_i64(2);
        while lo.add(&Integer::one()).cmp(&hi) == std::cmp::Ordering::Less {
            let mid = lo.add(&hi).div(&two).expect("divisor two");
            if mid.mul(&mid) <= *self {
                lo = mid;
            }
            else {
                hi = mid;
            }
        }
        Ok(lo)
    }

    /// 非负整数的精确 `n` 次根。若不是完全方幂则 `Ok(None)`。`n == 0` 为域错误。
    pub fn int_nth_root(&self, n: u32) -> Result<Option<Self>> {
        if n == 0 {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "int_nth_root")
                .detail("reason", "zero_index"));
        }
        if self.is_negative() {
            return Err(Diagnostic::new(DiagnosticCode::DomainError)
                .detail("domain", "numeric")
                .detail("operation", "int_nth_root")
                .detail("reason", "negative"));
        }
        if n == 1 {
            return Ok(Some(self.try_clone_in(&NumericContext::portable_default())?));
        }
        if self.is_zero() {
            return Ok(Some(Self::zero()));
        }
        if self.is_one() {
            return Ok(Some(Self::one()));
        }
        if n == 2 {
            let root = self.int_sqrt()?;
            return Ok(if root.mul(&root) == *self { Some(root) } else { None });
        }
        let mut lo = Self::one();
        let mut hi = self.try_clone_in(&NumericContext::portable_default())?.add(&Self::one());
        let two = Integer::from_i64(2);
        while lo.add(&Integer::one()).cmp(&hi) == std::cmp::Ordering::Less {
            let mid = lo.add(&hi).div(&two).expect("divisor two");
            let powered = match mid.pow_u32(n) {
                Ok(p) => p,
                Err(()) => {
                    hi = mid;
                    continue;
                }
            };
            match powered.cmp(self) {
                std::cmp::Ordering::Less => lo = mid,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Ok(Some(mid)),
            }
        }
        let powered = lo.pow_u32(n).map_err(|_| {
            Diagnostic::new(DiagnosticCode::ExponentOutOfRange)
                .detail("domain", "numeric")
                .detail("operation", "int_nth_root")
        })?;
        Ok(if powered == *self { Some(lo) } else { None })
    }

    /// 二进制 wire 符号码：`0` 零 · `1` 正 · `2` 负。
    pub(crate) fn wire_sign_code(&self) -> u8 {
        match self.sign() {
            Sign::Zero => 0,
            Sign::Positive => 1,
            Sign::Negative => 2,
        }
    }

    /// 二进制 wire 的无符号幅度字节（只读 limb，不 owning 复制）。
    pub(crate) fn wire_magnitude_bytes(&self) -> Vec<u8> {
        Natural::wire_encode_limbs(self.as_limbs())
    }

    /// 由符号码 + 幅度字节解码二进制 wire 整数。
    pub(crate) fn from_wire_magnitude(sign: u8, mag_bytes: &[u8]) -> Result<Self> {
        let mag = Natural::wire_decode_magnitude(mag_bytes)?;
        Self::from_wire_parts(sign, mag)
    }

    /// 由符号码 + 已解码幅度解码（canonical：`sign=0` ⇔ mag 为零；禁止负零）。
    pub(crate) fn from_wire_parts(sign: u8, mag: Natural) -> Result<Self> {
        use crate::format::validation::{WireReject, reject_non_canonical};
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
    Diagnostic::new(DiagnosticCode::NumericDivisionByZero).detail("domain", "numeric").detail("operation", op)
}

#[inline]
fn limbs_to_u64(limbs: &[u64]) -> Option<u64> {
    match limb_kernel::effective_len(limbs) {
        0 => Some(0),
        1 => Some(limbs[0]),
        _ => None,
    }
}

#[inline]
fn limbs_to_u128(limbs: &[u64]) -> Option<u128> {
    match limb_kernel::effective_len(limbs) {
        0 => Some(0),
        1 => Some(limbs[0] as u128),
        2 => Some(limbs[0] as u128 | ((limbs[1] as u128) << 64)),
        _ => None,
    }
}

fn f64_represents_integer(f: f64, n: &Integer) -> bool {
    if !f.is_finite() {
        return false;
    }
    if n.is_zero() {
        return f == 0.0;
    }
    let u = match limbs_to_u128(n.as_limbs()) {
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
