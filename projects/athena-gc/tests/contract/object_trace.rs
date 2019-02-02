//! Object arena + tracing collect 合同。

use athena_gc::{EmptyObjectGraph, GcHeap, GcMode, GcObjectId, HeapBudget, ObjectGraph, RootKind, Tracer};

struct EdgeGraph {
    edges: Vec<(GcObjectId, GcObjectId)>,
}

impl ObjectGraph for EdgeGraph {
    fn trace_object(&self, id: GcObjectId, tracer: &mut dyn Tracer) {
        for (from, to) in &self.edges {
            if *from == id {
                tracer.mark_object(*to);
            }
        }
    }
}

#[test]
fn allocate_object_payload_and_release() {
    let rc = GcHeap::new(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Auto);
    let id = heap.allocate_object(32).expect("alloc object");
    {
        let payload = heap.object_payload_mut(id).expect("payload");
        payload.fill(0xAB);
    }
    assert_eq!(heap.object_payload(id).expect("ro")[0], 0xAB);
    heap.release_object(id).expect("release");
    assert!(heap.object_payload(id).is_err());
    let _ = heap.collect().expect("collect");
}

#[test]
fn tracing_collect_sweeps_unreachable_object() {
    let rc = GcHeap::new(HeapBudget::default());
    let mut heap = rc.borrow_mut();
    heap.gc().set_base_mode(GcMode::Deferred);

    let root = heap.allocate_object(8).expect("root");
    let child = heap.allocate_object(8).expect("child");
    let orphan = heap.allocate_object(8).expect("orphan");

    let token = heap.roots_mut().register(root, RootKind::Session);
    let graph = EdgeGraph { edges: vec![(root, child)] };

    let report = heap.collect_traced(&graph).expect("collect");
    assert!(report.objects_swept >= 1);
    assert!(heap.object_payload(root).is_ok());
    assert!(heap.object_payload(child).is_ok());
    assert!(heap.object_payload(orphan).is_err());

    assert!(heap.roots_mut().unregister(token));
    let _ = heap.collect_traced(&EmptyObjectGraph).expect("collect2");
    assert!(heap.object_payload(root).is_err());
    assert!(heap.object_payload(child).is_err());
}
