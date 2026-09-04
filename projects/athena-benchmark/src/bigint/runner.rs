//! 统一运行器：按 layer / context_policy 执行单次操作。

use std::hint::black_box;

use athena_engine::runtime::values::numeric_clone::{clone_integer, clone_natural};
use athena_gc::HeapBudget;
use athena_numeric::{EphemeralInteger, EphemeralNatural, Integer, NumericContext, natural::Natural, number_from_wire};
use athena_types::wire::WireNumber;

use super::{
    BenchCase, BenchLayer, BigIntOp, ContextPolicy, Implementation,
    operands::{needs_product, operand_strings, pow_exp},
};

/// 已解析、可重复执行的用例（输入与 context 均在热路径外）。
pub struct BigIntPrepared {
    case: BenchCase,
    ctx: Option<NumericContext>,
    /// Numeric `Add`/`Mul` 校验 promote 目标（与 batch heap **不同**）。
    persist_ctx: Option<NumericContext>,
    athena_int: Option<AthenaIntOps>,
    athena_nat: Option<AthenaNatOps>,
    #[cfg(feature = "compare-num-bigint")]
    num: Option<NumOps>,
    #[cfg(feature = "compare-ibig")]
    ibig: Option<IbigOps>,
    #[cfg(feature = "compare-malachite")]
    malachite: Option<MalachiteOps>,
    expected_decimal: String,
}

struct AthenaIntOps {
    a: Integer,
    b: Integer,
    prod: Option<Integer>,
    exp: u32,
}

struct AthenaNatOps {
    a: Natural,
    b: Natural,
    prod: Option<Natural>,
    exp: u32,
}

#[cfg(feature = "compare-num-bigint")]
struct NumOps {
    a: num_bigint::BigInt,
    b: num_bigint::BigInt,
    prod: Option<num_bigint::BigInt>,
    exp: u32,
}

#[cfg(feature = "compare-ibig")]
struct IbigOps {
    a: ibig::IBig,
    b: ibig::IBig,
    prod: Option<ibig::IBig>,
    exp: u32,
}

#[cfg(feature = "compare-malachite")]
struct MalachiteOps {
    a: malachite::Integer,
    b: malachite::Integer,
    prod: Option<malachite::Integer>,
    exp: u32,
}

impl BigIntPrepared {
    /// 用例元数据。
    pub fn case(&self) -> BenchCase {
        self.case
    }

    /// 期望十进制（Athena numeric 参考结果）。
    pub fn expected_decimal(&self) -> &str {
        &self.expected_decimal
    }

    /// 正确性校验：执行一次并与参考十进制比对。
    pub fn validate(&self) -> Result<(), String> {
        let got = self.run_once_decimal();
        if got != self.expected_decimal {
            return Err(format!("expected {}, got {got}", self.expected_decimal));
        }
        Ok(())
    }

    /// 热路径单次（供计时；结果 `black_box`）。
    pub fn run_once(&self) {
        match self.case.implementation {
            Implementation::Athena => match self.case.layer {
                BenchLayer::Kernel => {
                    let _ = black_box(self.run_athena_kernel());
                }
                BenchLayer::Numeric => match self.case.operation {
                    BigIntOp::Add | BigIntOp::Mul => {
                        let _ = black_box(self.run_numeric_ephemeral_promote());
                    }
                    _ => {
                        let _ = black_box(self.run_athena_integer());
                    }
                },
                BenchLayer::E2e => {
                    let _ = black_box(self.run_athena_integer());
                }
                BenchLayer::Peer => unreachable!("athena is not peer"),
            },
            #[cfg(feature = "compare-num-bigint")]
            Implementation::Num => {
                let _ = black_box(self.run_num());
            }
            #[cfg(feature = "compare-ibig")]
            Implementation::Ibig => {
                let _ = black_box(self.run_ibig());
            }
            #[cfg(feature = "compare-malachite")]
            Implementation::Malachite => {
                let _ = black_box(self.run_malachite());
            }
            #[allow(unreachable_patterns)]
            _ => panic!("implementation disabled at compile time"),
        }
    }

    /// Criterion 批跑。
    ///
    /// - **Numeric `Add`/`Mul`**：单个 [`athena_gc::NumericBatch`] + `Ephemeral*`（不 promote）。
    /// - **Kernel** 与 Numeric 其余 op：热路径内临时 `bump_ephemeral` + mark/clear（操作数在 prepare 时 Full 发布，避免 Drop UAF）。
    /// - **e2e** / peer：真实 owning / Drop 路径。
    pub fn run_timed_batch(&self, iters: u64) -> std::time::Duration {
        use std::time::Instant;
        if self.case.implementation == Implementation::Athena
            && self.case.layer == BenchLayer::Numeric
            && matches!(self.case.operation, BigIntOp::Add | BigIntOp::Mul)
        {
            return self.run_numeric_ephemeral_timed(iters);
        }
        if let Some(ctx) = self.ctx.as_ref() {
            let heap = ctx.heap().clone();
            let mark = {
                let mut h = heap.borrow_mut();
                h.enable_bump_ephemeral(true);
                h.mark_numeric_bump()
            };
            let start = Instant::now();
            for _ in 0..iters {
                self.run_once();
            }
            let elapsed = start.elapsed();
            let mut h = heap.borrow_mut();
            h.clear_numeric_to(mark).expect("bump clear");
            h.enable_bump_ephemeral(false);
            elapsed
        }
        else {
            let start = Instant::now();
            for _ in 0..iters {
                self.run_once();
            }
            start.elapsed()
        }
    }

    /// Numeric `Add`/`Mul`：整批一个 `NumericBatch`，临时结果不 promote。
    fn run_numeric_ephemeral_timed(&self, iters: u64) -> std::time::Duration {
        use std::time::Instant;
        let ops = self.athena_int.as_ref().expect("athena int ops");
        let a_limbs = ops.a.as_limbs();
        let b_limbs = ops.b.as_limbs();
        let a_neg = ops.a.is_negative();
        let b_neg = ops.b.is_negative();
        let heap = self.ctx.as_ref().expect("numeric ctx").heap();
        let mut h = heap.borrow_mut();
        let start = Instant::now();
        h.with_numeric_batch(|batch| {
            for _ in 0..iters {
                match self.case.operation {
                    BigIntOp::Add => {
                        let e = EphemeralInteger::try_add(a_limbs, a_neg, b_limbs, b_neg, batch).expect("ephemeral add");
                        black_box(e.as_limbs());
                        drop(e);
                    }
                    BigIntOp::Mul => {
                        let e = EphemeralNatural::try_mul_schoolbook(a_limbs, b_limbs, batch).expect("ephemeral mul");
                        black_box(e.as_limbs());
                        drop(e);
                    }
                    _ => unreachable!("ephemeral timed only Add/Mul"),
                }
            }
        })
        .expect("numeric batch");
        start.elapsed()
    }

    /// Numeric `Add`/`Mul`：batch 内算完后 promote 到 `persist_ctx`（校验 / 单次路径）。
    fn run_numeric_ephemeral_promote(&self) -> Integer {
        let ops = self.athena_int.as_ref().expect("athena int ops");
        let persist = self.persist_ctx.as_ref().expect("persist ctx");
        let heap = self.ctx.as_ref().expect("numeric ctx").heap();
        let mut out: Option<Integer> = None;
        heap.borrow_mut()
            .with_numeric_batch(|batch| match self.case.operation {
                BigIntOp::Add => {
                    let e = EphemeralInteger::try_add(ops.a.as_limbs(), ops.a.is_negative(), ops.b.as_limbs(), ops.b.is_negative(), batch)
                        .expect("ephemeral add");
                    out = Some(e.promote(persist).expect("promote add"));
                }
                BigIntOp::Mul => {
                    let e = EphemeralNatural::try_mul_schoolbook(ops.a.as_limbs(), ops.b.as_limbs(), batch).expect("ephemeral mul");
                    let n = e.promote(persist).expect("promote mul mag");
                    let mut i = Integer::from_limbs_in(persist, n.as_limbs()).expect("int from limbs");
                    if ops.a.is_negative() != ops.b.is_negative() && !i.is_zero() {
                        i = i.neg();
                    }
                    out = Some(i);
                }
                _ => panic!("ephemeral promote only for Add/Mul"),
            })
            .expect("numeric batch promote");
        out.expect("promoted Integer")
    }

    fn run_once_decimal(&self) -> String {
        match self.case.implementation {
            Implementation::Athena => match self.case.layer {
                BenchLayer::Kernel => self.run_athena_kernel().to_decimal_string(),
                BenchLayer::Numeric => match self.case.operation {
                    BigIntOp::Add | BigIntOp::Mul => self.run_numeric_ephemeral_promote().to_decimal_string(),
                    _ => self.run_athena_integer().to_decimal_string(),
                },
                BenchLayer::E2e => self.run_athena_integer().to_decimal_string(),
                BenchLayer::Peer => unreachable!(),
            },
            #[cfg(feature = "compare-num-bigint")]
            Implementation::Num => self.run_num().to_str_radix(10),
            #[cfg(feature = "compare-ibig")]
            Implementation::Ibig => self.run_ibig().to_string(),
            #[cfg(feature = "compare-malachite")]
            Implementation::Malachite => self.run_malachite().to_string(),
            #[allow(unreachable_patterns)]
            _ => String::new(),
        }
    }

    fn run_athena_integer(&self) -> Integer {
        let ops = self.athena_int.as_ref().expect("athena int ops");
        match (self.case.layer, self.case.context_policy) {
            (BenchLayer::Numeric, ContextPolicy::Reused) => {
                let ctx = self.ctx.as_ref().expect("reused ctx");
                match self.case.operation {
                    BigIntOp::Add => ops.a.try_add(&ops.b, ctx).expect("add"),
                    BigIntOp::Mul => ops.a.try_mul(&ops.b, ctx).expect("mul"),
                    BigIntOp::Div => ops.prod.as_ref().expect("prod").try_div_rem_trunc(&ops.a, ctx).expect("div").0,
                    BigIntOp::Gcd => ops.a.try_gcd(&ops.b, ctx).expect("gcd"),
                    BigIntOp::Pow => pow_u32_reused(&ops.a, ops.exp, ctx),
                }
            }
            (BenchLayer::E2e, ContextPolicy::PerCall) => match self.case.operation {
                BigIntOp::Add => ops.a.add(&ops.b),
                BigIntOp::Mul => ops.a.mul(&ops.b),
                BigIntOp::Div => ops.prod.as_ref().expect("prod").div(&ops.a).expect("div"),
                BigIntOp::Gcd => ops.a.gcd(&ops.b),
                BigIntOp::Pow => ops.a.pow_u32(ops.exp).expect("pow"),
            },
            _ => panic!("unsupported athena integer layer/policy"),
        }
    }

    fn run_athena_kernel(&self) -> Natural {
        let ops = self.athena_nat.as_ref().expect("athena nat ops");
        let ctx = self.ctx.as_ref().expect("kernel ctx");
        match self.case.operation {
            BigIntOp::Add => ops.a.try_add(&ops.b, ctx).expect("add"),
            BigIntOp::Mul => ops.a.try_mul(&ops.b, ctx).expect("mul"),
            BigIntOp::Div => ops.prod.as_ref().expect("prod").try_div_rem(&ops.a, ctx).expect("div").0,
            BigIntOp::Gcd => ops.a.try_gcd(&ops.b, ctx).expect("gcd"),
            BigIntOp::Pow => nat_pow_u32(&ops.a, ops.exp, ctx),
        }
    }

    #[cfg(feature = "compare-num-bigint")]
    fn run_num(&self) -> num_bigint::BigInt {
        use num_traits::pow::Pow;
        let ops = self.num.as_ref().expect("num ops");
        match self.case.operation {
            BigIntOp::Add => &ops.a + &ops.b,
            BigIntOp::Mul => &ops.a * &ops.b,
            BigIntOp::Div => ops.prod.as_ref().expect("prod") / &ops.a,
            BigIntOp::Gcd => gcd_num(&ops.a, &ops.b),
            BigIntOp::Pow => Pow::pow(&ops.a, ops.exp),
        }
    }

    #[cfg(feature = "compare-ibig")]
    fn run_ibig(&self) -> ibig::IBig {
        let ops = self.ibig.as_ref().expect("ibig ops");
        match self.case.operation {
            BigIntOp::Add => &ops.a + &ops.b,
            BigIntOp::Mul => &ops.a * &ops.b,
            BigIntOp::Div => ops.prod.as_ref().expect("prod") / &ops.a,
            BigIntOp::Gcd => gcd_ibig(&ops.a, &ops.b),
            BigIntOp::Pow => ops.a.pow(usize::try_from(ops.exp).expect("exp fits usize")),
        }
    }

    #[cfg(feature = "compare-malachite")]
    fn run_malachite(&self) -> malachite::Integer {
        use malachite::base::num::arithmetic::traits::Pow as MalachitePow;
        let ops = self.malachite.as_ref().expect("malachite ops");
        match self.case.operation {
            BigIntOp::Add => &ops.a + &ops.b,
            BigIntOp::Mul => &ops.a * &ops.b,
            BigIntOp::Div => ops.prod.as_ref().expect("prod") / &ops.a,
            BigIntOp::Gcd => gcd_malachite(&ops.a, &ops.b),
            BigIntOp::Pow => MalachitePow::pow(ops.a.clone(), u64::from(ops.exp)),
        }
    }
}

/// 准备单条用例。
pub fn prepare(case: BenchCase) -> BigIntPrepared {
    assert!(case.implementation.feature_enabled(), "implementation feature disabled");

    let strings = operand_strings(case.bits);
    let exp = pow_exp(case.bits).exp;

    let ref_ctx = NumericContext::unlimited();
    let a_ref = integer_from_decimal(&strings.a);
    let b_ref = integer_from_decimal(&strings.b);
    let prod_ref = if needs_product(case.operation) { Some(a_ref.try_mul(&b_ref, &ref_ctx).expect("ref mul")) } else { None };
    let expected_decimal = reference_decimal(case.operation, &a_ref, &b_ref, prod_ref.as_ref(), exp, &ref_ctx);

    // 操作数 / prod 在 Full 路径发布；`bump_ephemeral` 只在 `run_timed_batch` / `NumericBatch` 热路径打开。
    // 若 prepare 阶段就开 ephemeral，长期持有的 prod/operand 会在 Drop 时空释放 → 进程退出 UAF。
    let ctx = match case.layer {
        BenchLayer::Kernel => Some(NumericContext::kernel_bench_with_heap_budget(HeapBudget::for_microbench())),
        BenchLayer::Numeric => Some(NumericContext::session_with_heap_budget(HeapBudget::for_microbench())),
        BenchLayer::E2e | BenchLayer::Peer => None,
    };

    let persist_ctx = match case.layer {
        BenchLayer::Numeric => Some(NumericContext::session_with_heap_budget(HeapBudget::for_microbench())),
        _ => None,
    };

    let athena_int = match case.implementation {
        Implementation::Athena if matches!(case.layer, BenchLayer::Numeric | BenchLayer::E2e) => {
            Some(AthenaIntOps { a: clone_integer(&a_ref), b: clone_integer(&b_ref), prod: prod_ref.as_ref().map(|n| clone_integer(n)), exp })
        }
        _ => None,
    };

    let athena_nat = match (case.implementation, case.layer) {
        (Implementation::Athena, BenchLayer::Kernel) => {
            let ctx = ctx.as_ref().expect("kernel ctx");
            // 落在 kernel 隔离 heap（Full），避免 `from_str` 污染 `shared_default`。
            let a = Natural::from_limbs_in(ctx, a_ref.as_limbs().to_vec()).expect("natural a");
            let b = Natural::from_limbs_in(ctx, b_ref.as_limbs().to_vec()).expect("natural b");
            let prod = if needs_product(case.operation) { Some(a.try_mul(&b, ctx).expect("nat prod")) } else { None };
            Some(AthenaNatOps { a, b, prod, exp })
        }
        _ => None,
    };

    #[cfg(feature = "compare-num-bigint")]
    let num = if case.implementation == Implementation::Num {
        use num_traits::Num;
        let a = num_bigint::BigInt::from_str_radix(&strings.a, 10).expect("num a");
        let b = num_bigint::BigInt::from_str_radix(&strings.b, 10).expect("num b");
        let prod = if needs_product(case.operation) { Some(&a * &b) } else { None };
        Some(NumOps { a, b, prod, exp })
    }
    else {
        None
    };

    #[cfg(feature = "compare-ibig")]
    let ibig = if case.implementation == Implementation::Ibig {
        let a: ibig::IBig = strings.a.parse().expect("ibig a");
        let b: ibig::IBig = strings.b.parse().expect("ibig b");
        let prod = if needs_product(case.operation) { Some(&a * &b) } else { None };
        Some(IbigOps { a, b, prod, exp })
    }
    else {
        None
    };

    #[cfg(feature = "compare-malachite")]
    let malachite = if case.implementation == Implementation::Malachite {
        use std::str::FromStr;
        let a = malachite::Integer::from_str(&strings.a).expect("mal a");
        let b = malachite::Integer::from_str(&strings.b).expect("mal b");
        let prod = if needs_product(case.operation) { Some(&a * &b) } else { None };
        Some(MalachiteOps { a, b, prod, exp })
    }
    else {
        None
    };

    BigIntPrepared {
        case,
        ctx,
        persist_ctx,
        athena_int,
        athena_nat,
        #[cfg(feature = "compare-num-bigint")]
        num,
        #[cfg(feature = "compare-ibig")]
        ibig,
        #[cfg(feature = "compare-malachite")]
        malachite,
        expected_decimal,
    }
}

impl Drop for BigIntPrepared {
    fn drop(&mut self) {
        // 先放下 owning 幅度，再放 heap context，避免 bump/registry 时序踩踏。
        self.athena_int = None;
        self.athena_nat = None;
        #[cfg(feature = "compare-num-bigint")]
        {
            self.num = None;
        }
        #[cfg(feature = "compare-ibig")]
        {
            self.ibig = None;
        }
        #[cfg(feature = "compare-malachite")]
        {
            self.malachite = None;
        }
        self.persist_ctx = None;
        self.ctx = None;
    }
}

/// 准备完整矩阵。
pub fn prepare_all() -> Vec<BigIntPrepared> {
    super::all_cases().into_iter().map(prepare).collect()
}

fn integer_from_decimal(s: &str) -> Integer {
    number_from_wire(&WireNumber::from_decimal_str(s).expect("wire decimal"))
        .expect("from wire")
        .as_integer()
        .map(clone_integer)
        .expect("integer")
}

fn reference_decimal(op: BigIntOp, a: &Integer, b: &Integer, prod: Option<&Integer>, exp: u32, ctx: &NumericContext) -> String {
    let v = match op {
        BigIntOp::Add => a.try_add(b, ctx).expect("ref add"),
        BigIntOp::Mul => a.try_mul(b, ctx).expect("ref mul"),
        BigIntOp::Div => prod.expect("prod").try_div_rem_trunc(a, ctx).expect("ref div").0,
        BigIntOp::Gcd => a.try_gcd(b, ctx).expect("ref gcd"),
        BigIntOp::Pow => pow_u32_reused(a, exp, ctx),
    };
    v.to_decimal_string()
}

fn pow_u32_reused(base: &Integer, exp: u32, ctx: &NumericContext) -> Integer {
    if exp == 0 {
        return Integer::one();
    }
    if base.is_zero() {
        return Integer::zero();
    }
    let mut acc = Integer::one();
    let mut cur = clone_integer(base);
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            acc = acc.try_mul(&cur, ctx).expect("pow mul");
        }
        cur = cur.try_mul(&cur, ctx).expect("pow square");
        e >>= 1;
    }
    acc
}

fn nat_pow_u32(base: &Natural, exp: u32, ctx: &NumericContext) -> Natural {
    if exp == 0 {
        return Natural::one();
    }
    if base.is_zero() {
        return Natural::zero();
    }
    let mut acc = Natural::one();
    let mut cur = clone_natural(base);
    let mut e = exp;
    while e > 0 {
        if e & 1 == 1 {
            acc = acc.try_mul(&cur, ctx).expect("pow mul");
        }
        cur = cur.try_mul(&cur, ctx).expect("pow square");
        e >>= 1;
    }
    acc
}

#[cfg(feature = "compare-num-bigint")]
fn gcd_num(a: &num_bigint::BigInt, b: &num_bigint::BigInt) -> num_bigint::BigInt {
    use num_traits::{Signed, Zero};
    let mut x = a.abs();
    let mut y = b.abs();
    while !y.is_zero() {
        let r = &x % &y;
        x = y;
        y = r;
    }
    x
}

#[cfg(feature = "compare-ibig")]
fn gcd_ibig(a: &ibig::IBig, b: &ibig::IBig) -> ibig::IBig {
    use ibig::ops::Abs as IbigAbs;
    let zero = ibig::IBig::from(0);
    let mut x = IbigAbs::abs(a.clone());
    let mut y = IbigAbs::abs(b.clone());
    while y != zero {
        let r = &x % &y;
        x = y;
        y = r;
    }
    x
}

#[cfg(feature = "compare-malachite")]
fn gcd_malachite(a: &malachite::Integer, b: &malachite::Integer) -> malachite::Integer {
    use malachite::base::num::arithmetic::traits::Abs as MalachiteAbs;
    let zero = malachite::Integer::from(0);
    let mut x = MalachiteAbs::abs(a.clone());
    let mut y = MalachiteAbs::abs(b.clone());
    while y != zero {
        let r = &x % &y;
        x = y;
        y = r;
    }
    x
}
