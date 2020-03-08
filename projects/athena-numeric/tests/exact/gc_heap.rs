//! Heap → `athena-gc` 分配与 Trace 合同。

use athena_gc::{EmptyObjectGraph, GcHeap, GcMode, HeapBudget, MarkState, RootKind, Trace, Tracer};
use athena_numeric::{NumericContext, execution_budget::ExecutionBudget, natural::Natural};

struct CaptureAllocs {
    marked: Vec<usize>,
}

impl Tracer for CaptureAllocs {
    fn mark_object(&mut self, _id: athena_gc::GcObjectId) {}

    fn mark_allocation(&mut self, payload: *const u8) {
        self.marked.push(payload as usize);
    }
}

#[test]
fn heap_natural_allocates_on_ctx_heap_and_traces() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    {
        let h = heap.borrow_mut();
        h.gc().set_base_mode(GcMode::Deferred);
    }
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap.clone());

    // ≥3 limbs → Heap magnitude
    let limbs = vec![1u64, 2, 3, 4];
    let n = Natural::from_limbs_in(&ctx, limbs).expect("alloc");
    let mut cap = CaptureAllocs { marked: Vec::new() };
    n.trace(&mut cap);
    assert!(!cap.marked.is_empty(), "Heap Natural must mark limb allocation");

    let before = heap.borrow().resident_bytes();
    assert!(before > 0);
    drop(n);
    let report = heap.borrow_mut().collect().expect("collect");
    assert!(report.segments_reclaimed >= 1 || heap.borrow().resident_bytes() < before);
}

#[test]
fn bump_ephemeral_clear_rewinds_without_drop_tax() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    heap.borrow_mut().enable_bump_ephemeral(true);
    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap.clone());
    let a = Natural::from_limbs_in(&ctx, vec![1, 2, 3]).expect("a");
    let b = Natural::from_limbs_in(&ctx, vec![4, 5, 6]).expect("b");
    let mark = heap.borrow().mark_numeric_bump();
    let used_before = heap.borrow().segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum::<usize>();
    for _ in 0..64 {
        let _ = a.try_add(&b, &ctx).expect("add");
    }
    let used_mid = heap.borrow().segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum::<usize>();
    assert!(used_mid > used_before, "ephemeral bump must advance");
    heap.borrow_mut().clear_numeric_to(mark).expect("clear");
    let used_after = heap.borrow().segments().filter(|s| s.kind == athena_gc::SegmentKind::Numeric).map(|s| s.used).sum::<usize>();
    assert_eq!(used_after, used_before, "clear must rewind to mark");
    // 操作数仍可读
    assert_eq!(a.as_limbs(), &[1, 2, 3]);
    assert_eq!(b.as_limbs(), &[4, 5, 6]);
}

#[test]
fn isolated_heap_not_shared_default() {
    let heap_a = GcHeap::new_shared(HeapBudget::default());
    let heap_b = GcHeap::new_shared(HeapBudget::default());
    assert_ne!(heap_a.borrow().id(), heap_b.borrow().id());

    let ctx = NumericContext::with_heap(ExecutionBudget::unlimited(), heap_a.clone());
    let n = Natural::from_limbs_in(&ctx, vec![9, 8, 7]).expect("alloc");
    drop(n);
    let _ = heap_a.borrow_mut().collect();
    assert_eq!(heap_b.borrow().resident_bytes(), 0);
}

#[test]
fn object_root_keeps_payload_across_collect() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Deferred);
    let obj = heap.allocate_object(16).expect("obj");
    heap.object_payload_mut(obj).expect("w")[0] = 7;
    let token = heap.roots_mut().register(obj, RootKind::UserRetain);
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect");
    assert_eq!(report.objects_swept, 0);
    assert_eq!(heap.object_payload(obj).expect("ro")[0], 7);
    assert!(heap.roots_mut().unregister(token));
    let report = heap.collect_traced(&EmptyObjectGraph).expect("collect2");
    assert!(report.objects_swept >= 1);
    assert!(heap.object_payload(obj).is_err());
}

#[test]
fn mark_state_black_after_mark_limbs() {
    let rc = GcHeap::new_shared(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    let block = heap.allocate_numeric_block(4).expect("block");
    heap.mark_limbs(block.ptr).expect("mark");
    let hdr = heap.header_for_limbs(block.ptr).expect("hdr");
    assert_eq!(hdr.mark_state, MarkState::Black);
    heap.release_numeric_block(block).expect("rel");
}

#[test]
fn session_default_is_isolated_deferred_not_shared_auto() {
    let shared = GcHeap::shared_default();
    let ctx = NumericContext::session_default();
    assert_ne!(ctx.heap().borrow().id(), shared.borrow().id());
    assert_eq!(ctx.heap().borrow().effective_mode(), GcMode::Deferred);
    assert_eq!(shared.borrow().effective_mode(), GcMode::Auto);
}

#[test]
fn session_default_try_add_publishes_on_session_heap() {
    use athena_numeric::Integer;

    let ctx = NumericContext::session_default();
    let shared_before = GcHeap::shared_default().borrow().resident_bytes();
    let limbs = [1u64, 2, 3, 4, 5];
    let a = Integer::from_limbs_in(&ctx, limbs).expect("a");
    let b = Integer::from_limbs_in(&ctx, limbs).expect("b");
    let sum = a.try_add(&b, &ctx).expect("add");
    assert!(!sum.is_zero());

    let mut cap = CaptureAllocs { marked: Vec::new() };
    sum.trace(&mut cap);
    assert!(!cap.marked.is_empty(), "heap result must mark session allocation");
    assert!(ctx.heap().borrow().resident_bytes() > 0);
    // Living 18：结果发布在隔离 session 堆，不得污染 shared Auto。
    assert_eq!(GcHeap::shared_default().borrow().resident_bytes(), shared_before);
}

#[test]
fn session_default_publishes_tracing_sweep_heap() {
    use athena_numeric::natural::Natural;

    let ctx = NumericContext::session_default();
    let n = Natural::from_limbs_in(&ctx, vec![1, 2, 3, 4]).expect("heap natural");
    let ptr = n.as_limbs().as_ptr();
    let nn = core::ptr::NonNull::new(ptr as *mut u64).expect("non-null limbs");
    assert!(ctx.heap().borrow().may_root_numeric(nn).expect("rootable"));
    assert!(!ctx.heap().borrow().may_explicit_release_numeric(nn).expect("not temp"));
    assert_eq!(ctx.heap().borrow().roots().numeric_len(), 1);
}

#[test]
fn session_tracing_try_clone_in_is_deep_copy() {
    use athena_numeric::natural::Natural;
    use core::ptr::NonNull;

    let ctx = NumericContext::session_default();
    let n = Natural::from_limbs_in(&ctx, vec![9, 8, 7, 6]).expect("n");
    let roots_before = ctx.heap().borrow().roots().numeric_len();
    let cloned = n.try_clone_in(&ctx).expect("deep copy");
    assert_eq!(n.as_limbs(), cloned.as_limbs());
    assert_ne!(n.as_limbs().as_ptr(), cloned.as_limbs().as_ptr(), "Living 31: try_clone_in must deep-copy Heap");
    // Living 31：深复制为新 PublishedNumericBlock + root。
    assert_eq!(ctx.heap().borrow().roots().numeric_len(), roots_before + 1);
    let cptr = NonNull::new(cloned.as_limbs().as_ptr() as *mut u64).expect("ptr");
    assert!(ctx.heap().borrow().may_root_numeric(cptr).expect("published"));
    assert!(!ctx.heap().borrow().may_explicit_release_numeric(cptr).expect("not temp"));
}

#[test]
fn portable_default_also_publishes_tracing_sweep_heap() {
    use athena_numeric::natural::Natural;

    // Living 31：删除 publishes_gc_owned 双轨；portable 持久发布同样是 TracingSweep。
    let ctx = NumericContext::portable_default();
    let n = Natural::from_limbs_in(&ctx, vec![1, 2, 3, 4]).expect("heap natural");
    let ptr = n.as_limbs().as_ptr();
    let nn = core::ptr::NonNull::new(ptr as *mut u64).expect("non-null limbs");
    assert!(ctx.heap().borrow().may_root_numeric(nn).expect("rootable"));
    assert!(!ctx.heap().borrow().may_explicit_release_numeric(nn).expect("not temp"));
}

#[test]
fn portable_and_session_heap_naturals_are_rooted_published() {
    use athena_numeric::natural::Natural;
    use core::ptr::NonNull;

    for ctx in [NumericContext::portable_default(), NumericContext::session_default()] {
        let n = Natural::from_limbs_in(&ctx, vec![3, 4, 5, 6]).expect("heap natural");
        let nn = NonNull::new(n.as_limbs().as_ptr() as *mut u64).expect("ptr");
        assert!(ctx.heap().borrow().may_root_numeric(nn).expect("rootable"));
        assert!(!ctx.heap().borrow().may_explicit_release_numeric(nn).expect("not ExplicitRelease"));
        let cloned = n.try_clone_in(&ctx).expect("clone");
        let cn = NonNull::new(cloned.as_limbs().as_ptr() as *mut u64).expect("cptr");
        assert!(ctx.heap().borrow().may_root_numeric(cn).expect("clone published"));
    }
}

#[test]
fn session_rooted_drop_unregisters_without_free_mismatch() {
    use athena_numeric::natural::Natural;

    let ctx = NumericContext::session_default();
    let n = Natural::from_limbs_in(&ctx, vec![1, 2, 3, 4]).expect("heap natural");
    let roots_before = ctx.heap().borrow().roots().numeric_len();
    assert_eq!(roots_before, 1);
    drop(n);
    assert_eq!(ctx.heap().borrow().roots().numeric_len(), 0);
    assert_eq!(ctx.heap().borrow().stats().lifecycle_mismatch, 0);
}
