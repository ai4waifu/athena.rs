//! Path-segment fixtures inside `athena-benchmark` only.
//!
//! Uses public Athena APIs plus a local stack limb add as a lower-bound reference.
//! Does **not** add hooks into `athena-numeric`.

use std::{hint::black_box, str::FromStr};

use athena_gc::{GcHeap, GcMode, HeapBudget};
use athena_numeric::{ExecutionBudget, Integer, NumericContext, natural::Natural};

use crate::{
    fixture::{BenchGroup, Fixture, FixtureMeta, Suite},
    validate::{DeterminacyKind, ExactnessKind, ValidationSummary},
};

const LIMBS4: [u64; 4] = [0x1111_1111_1111_1111, 0x2222_2222_2222_2222, 0x3333_3333_3333_3333, 0x4444_4444_4444_4444];
const LIMBS4_B: [u64; 4] =
    [0x5555_5555_5555_5555, 0x6666_6666_6666_6666, 0x7777_7777_7777_7777, 0x0000_0000_0000_0001];

/// 每个 `run_once` 内重复次数，摊薄 `Instant` 底噪。
const BATCH: u32 = 2_000;

fn stack_add4(a: &[u64; 4], b: &[u64; 4]) -> [u64; 5] {
    let mut out = [0u64; 5];
    let mut carry = 0u64;
    for i in 0..4 {
        let (s, c1) = a[i].overflowing_add(b[i]);
        let (s, c2) = s.overflowing_add(carry);
        out[i] = s;
        carry = u64::from(c1) + u64::from(c2);
    }
    out[4] = carry;
    out
}

struct CtxBundle {
    ctx: NumericContext,
}

fn make_ctx(mode: GcMode) -> CtxBundle {
    let heap = GcHeap::new_shared(HeapBudget::default());
    heap.borrow().gc().set_base_mode(mode);
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    CtxBundle { ctx }
}

/// Shared-default context（与 `Integer::from_str` 同堆）。
fn shared_ctx() -> NumericContext {
    NumericContext::unlimited()
}

struct StackAdd4;
impl Fixture for StackAdd4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.stack_add_4", BenchGroup::Path, "4_limbs", "stack_floor")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "stack add4 floor"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(stack_add4(&LIMBS4, &LIMBS4_B));
        }
    }
}

struct AllocBlock4 {
    bundle: CtxBundle,
}
impl Fixture for AllocBlock4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.alloc_numeric_block_4", BenchGroup::Path, "4_limbs", "gc_heap")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let block = self.bundle.ctx.allocate_numeric_block(4).map_err(|d| d.code.as_str().to_string())?;
        self.bundle.ctx.heap().borrow_mut().release_numeric_block(block).map_err(|e| e.to_string())?;
        Ok(ValidationSummary::passed(
            ExactnessKind::Unspecified,
            DeterminacyKind::Deterministic,
            "allocate+release",
        ))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            let block = self.bundle.ctx.allocate_numeric_block(4).expect("alloc");
            black_box(block.capacity);
            self.bundle.ctx.heap().borrow_mut().release_numeric_block(block).expect("release");
        }
    }
}

struct PublishLimbs4 {
    bundle: CtxBundle,
}
impl Fixture for PublishLimbs4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.publish_from_limbs_4", BenchGroup::Path, "4_limbs", "natural_publish")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let n = Natural::from_limbs_in(&self.bundle.ctx, LIMBS4.to_vec()).map_err(|d| d.code.as_str().to_string())?;
        if n.as_limbs() != LIMBS4 {
            return Err(format!("mismatch {:?}", n.as_limbs()));
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "from_limbs_in"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(Natural::from_limbs_in(&self.bundle.ctx, LIMBS4.to_vec()).expect("publish"));
        }
    }
}

struct CloneHeapNatural {
    bundle: CtxBundle,
    n: Natural,
}
impl Fixture for CloneHeapNatural {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.clone_heap_natural_4", BenchGroup::Path, "4_limbs", "magnitude_clone")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        if self.n.as_limbs() != LIMBS4 {
            return Err("clone src corrupted".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Natural clone"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(self.n.clone());
        }
    }
}

struct ScratchFrameEmpty {
    bundle: CtxBundle,
}
impl Fixture for ScratchFrameEmpty {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.scratch_frame_empty", BenchGroup::Path, "n/a", "scratch")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        Ok(ValidationSummary::passed(ExactnessKind::Unspecified, DeterminacyKind::Deterministic, "scratch"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            self.bundle.ctx.with_scratch_frame(|scratch, _| {
                let _ = scratch.mark();
            });
        }
    }
}

struct NaturalTryAdd4 {
    bundle: CtxBundle,
    a: Natural,
    b: Natural,
}
impl Fixture for NaturalTryAdd4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.natural_try_add_4", BenchGroup::Path, "4_limbs", "natural_heap")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        if self.a.as_limbs() != LIMBS4 || self.b.as_limbs() != LIMBS4_B {
            return Err(format!("corrupted a={:?} b={:?}", self.a.as_limbs(), self.b.as_limbs()));
        }
        let s = self.a.try_add(&self.b, &self.bundle.ctx).map_err(|d| d.code.as_str().to_string())?;
        if s.is_zero() {
            return Err("sum zero".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Natural::try_add"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(self.a.try_add(&self.b, &self.bundle.ctx).expect("add"));
        }
    }
}

struct IntegerClone4 {
    a: Integer,
}
impl Fixture for IntegerClone4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.integer_clone_4", BenchGroup::Path, "4_limbs", "integer_clone")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        if self.a.bits() < 200 {
            return Err(format!("expected ~256-bit, bits={}", self.a.bits()));
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Integer clone"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(self.a.clone());
        }
    }
}

struct IntegerTryAdd4 {
    ctx: NumericContext,
    a: Integer,
    b: Integer,
}
impl Fixture for IntegerTryAdd4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.integer_try_add_4", BenchGroup::Path, "4_limbs", "integer_shared_auto")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let s = self.a.try_add(&self.b, &self.ctx).map_err(|d| d.code.as_str().to_string())?;
        if s.is_zero() {
            return Err("sum zero".into());
        }
        Ok(ValidationSummary::passed(
            ExactnessKind::Exact,
            DeterminacyKind::Deterministic,
            &format!("Integer::try_add shared Auto bits={}", self.a.bits()),
        ))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(self.a.try_add(&self.b, &self.ctx).expect("add"));
        }
    }
}

/// Session-style Integer add：隔离 heap + `GcMode::Disabled`（Living 18 numeric 层）。
struct IntegerTryAddSession4 {
    bundle: CtxBundle,
    a: Integer,
    b: Integer,
}
impl Fixture for IntegerTryAddSession4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.integer_try_add_session_4", BenchGroup::Path, "4_limbs", "integer_session_disabled")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let s = self.a.try_add(&self.b, &self.bundle.ctx).map_err(|d| d.code.as_str().to_string())?;
        if s.is_zero() {
            return Err("sum zero".into());
        }
        Ok(ValidationSummary::passed(
            ExactnessKind::Exact,
            DeterminacyKind::Deterministic,
            &format!("Integer::try_add session Disabled bits={}", self.a.bits()),
        ))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(self.a.try_add(&self.b, &self.bundle.ctx).expect("add"));
        }
    }
}

struct IntegerAddE2e4 {
    a: Integer,
    b: Integer,
}
impl Fixture for IntegerAddE2e4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.integer_add_e2e_4", BenchGroup::Path, "4_limbs", "integer_per_call")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let s = self.a.add(&self.b);
        if s.is_zero() {
            return Err("sum zero".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Integer::add"))
    }
    fn run_once(&self) {
        for _ in 0..BATCH {
            black_box(self.a.add(&self.b));
        }
    }
}

pub(super) fn register(suite: &mut Suite) {
    suite.register(Box::new(StackAdd4));
    suite.register(Box::new(AllocBlock4 { bundle: make_ctx(GcMode::Disabled) }));

    {
        let bundle = make_ctx(GcMode::Disabled);
        suite.register(Box::new(PublishLimbs4 { bundle }));
    }
    {
        let bundle = make_ctx(GcMode::Disabled);
        let n = Natural::from_limbs_in(&bundle.ctx, LIMBS4.to_vec()).expect("n");
        suite.register(Box::new(CloneHeapNatural { bundle, n }));
    }
    suite.register(Box::new(ScratchFrameEmpty { bundle: make_ctx(GcMode::Disabled) }));
    {
        let bundle = make_ctx(GcMode::Disabled);
        let a = Natural::from_limbs_in(&bundle.ctx, LIMBS4.to_vec()).expect("a");
        let b = Natural::from_limbs_in(&bundle.ctx, LIMBS4_B.to_vec()).expect("b");
        suite.register(Box::new(NaturalTryAdd4 { bundle, a, b }));
    }

    let a_dec = Natural::from_limbs(LIMBS4.to_vec()).unwrap().to_decimal_string();
    let b_dec = Natural::from_limbs(LIMBS4_B.to_vec()).unwrap().to_decimal_string();
    let a_int = Integer::from_str(&a_dec).expect("a");
    let b_int = Integer::from_str(&b_dec).expect("b");
    suite.register(Box::new(IntegerClone4 { a: a_int.clone() }));
    suite.register(Box::new(IntegerTryAdd4 { ctx: shared_ctx(), a: a_int.clone(), b: b_int.clone() }));
    suite.register(Box::new(IntegerTryAddSession4 {
        bundle: make_ctx(GcMode::Disabled),
        a: a_int.clone(),
        b: b_int.clone(),
    }));
    suite.register(Box::new(IntegerAddE2e4 { a: a_int, b: b_int }));
}
