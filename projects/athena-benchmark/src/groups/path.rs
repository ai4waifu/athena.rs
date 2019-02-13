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

/// Local floor: 4-limb add into a stack buffer（不是 Athena kernel 合同，只作数量级下界）。
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

fn make_ctx_disabled() -> CtxBundle {
    let heap = GcHeap::new_shared(HeapBudget::default());
    heap.borrow().gc().set_base_mode(GcMode::Disabled);
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap);
    CtxBundle { ctx }
}

struct StackAdd4;
impl Fixture for StackAdd4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.stack_add_4", BenchGroup::Path, "4_limbs", "stack_floor")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let out = stack_add4(&LIMBS4, &LIMBS4_B);
        if out[0] == 0 && out[1] == 0 {
            return Err("unexpected zero".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "stack add4 floor"))
    }
    fn run_once(&self) {
        black_box(stack_add4(&LIMBS4, &LIMBS4_B));
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
        drop(block);
        Ok(ValidationSummary::passed(
            ExactnessKind::Unspecified,
            DeterminacyKind::Deterministic,
            "allocate_numeric_block(4)",
        ))
    }
    fn run_once(&self) {
        let block = self.bundle.ctx.allocate_numeric_block(4).expect("alloc");
        black_box(drop(block));
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
        if n.as_limbs().len() != 4 {
            return Err("publish len".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "from_limbs_in 4"))
    }
    fn run_once(&self) {
        black_box(Natural::from_limbs_in(&self.bundle.ctx, LIMBS4.to_vec()).expect("publish"));
    }
}

struct CloneHeapNatural {
    n: Natural,
}
impl Fixture for CloneHeapNatural {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.clone_heap_natural_4", BenchGroup::Path, "4_limbs", "magnitude_clone")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let c = self.n.clone();
        if c.as_limbs() != self.n.as_limbs() {
            return Err("clone mismatch".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Natural heap clone"))
    }
    fn run_once(&self) {
        black_box(self.n.clone());
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
        self.bundle.ctx.with_scratch_frame(|scratch, _| {
            let _ = scratch.mark();
        });
        Ok(ValidationSummary::passed(ExactnessKind::Unspecified, DeterminacyKind::Deterministic, "scratch frame"))
    }
    fn run_once(&self) {
        self.bundle.ctx.with_scratch_frame(|scratch, _| {
            let _ = scratch.mark();
        });
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
        let s = self.a.try_add(&self.b, &self.bundle.ctx).map_err(|d| d.code.as_str().to_string())?;
        if s.is_zero() {
            return Err("sum zero".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Natural::try_add heap"))
    }
    fn run_once(&self) {
        black_box(self.a.try_add(&self.b, &self.bundle.ctx).expect("add"));
    }
}

struct IntegerTryAdd4 {
    bundle: CtxBundle,
    a: Integer,
    b: Integer,
}
impl Fixture for IntegerTryAdd4 {
    fn meta(&self) -> FixtureMeta {
        FixtureMeta::basic("path.integer_try_add_4", BenchGroup::Path, "4_limbs", "integer_heap")
    }
    fn validate(&self) -> Result<ValidationSummary, String> {
        let s = self.a.try_add(&self.b, &self.bundle.ctx).map_err(|d| d.code.as_str().to_string())?;
        if s.is_zero() {
            return Err("sum zero".into());
        }
        Ok(ValidationSummary::passed(ExactnessKind::Exact, DeterminacyKind::Deterministic, "Integer::try_add"))
    }
    fn run_once(&self) {
        black_box(self.a.try_add(&self.b, &self.bundle.ctx).expect("add"));
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
        Ok(ValidationSummary::passed(
            ExactnessKind::Exact,
            DeterminacyKind::Deterministic,
            "Integer::add per-call ctx",
        ))
    }
    fn run_once(&self) {
        black_box(self.a.add(&self.b));
    }
}

pub(super) fn register(suite: &mut Suite) {
    let setup = make_ctx_disabled();
    let a_nat = Natural::from_limbs_in(&setup.ctx, LIMBS4.to_vec()).expect("a");
    let b_nat = Natural::from_limbs_in(&setup.ctx, LIMBS4_B.to_vec()).expect("b");
    let a_int = Integer::from_str(&a_nat.to_decimal_string()).expect("a int");
    let b_int = Integer::from_str(&b_nat.to_decimal_string()).expect("b int");

    suite.register(Box::new(StackAdd4));
    suite.register(Box::new(AllocBlock4 { bundle: make_ctx_disabled() }));
    suite.register(Box::new(PublishLimbs4 { bundle: make_ctx_disabled() }));
    suite.register(Box::new(CloneHeapNatural { n: a_nat.clone() }));
    suite.register(Box::new(ScratchFrameEmpty { bundle: make_ctx_disabled() }));
    suite.register(Box::new(NaturalTryAdd4 {
        bundle: make_ctx_disabled(),
        a: a_nat,
        b: b_nat,
    }));
    suite.register(Box::new(IntegerTryAdd4 {
        bundle: make_ctx_disabled(),
        a: a_int.clone(),
        b: b_int.clone(),
    }));
    suite.register(Box::new(IntegerAddE2e4 { a: a_int, b: b_int }));
}
