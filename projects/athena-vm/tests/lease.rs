//! `ExecutionLease` 合同。

use std::rc::Rc;

use athena_gc::{GcHeap, HeapBudget};
use athena_types::{TermId, TermRef};
use athena_vm::ExecutionLease;

#[test]
fn lease_registers_and_drops_object_root() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let object = {
        let mut h = heap.borrow_mut();
        h.allocate_object(8).expect("alloc")
    };
    {
        let mut lease = ExecutionLease::new(Rc::clone(&heap));
        let _token = lease.register_object(object);
        assert_eq!(lease.object_root_count(), 1);
        assert_eq!(heap.borrow().roots().iter().count(), 1);
    }
    assert_eq!(heap.borrow().roots().iter().count(), 0);
}

#[test]
fn lease_pins_term_refs_and_clears_on_drop() {
    let heap = GcHeap::new_shared(HeapBudget::default());
    let mut lease = ExecutionLease::new(Rc::clone(&heap));
    lease.register_term(TermRef::new(TermId(3), 1));
    lease.register_term(TermRef::new(TermId(7), 1));
    assert_eq!(lease.term_pin_count(), 2);
    assert_eq!(lease.term_pins(), &[TermRef::new(TermId(3), 1), TermRef::new(TermId(7), 1)]);
    lease.release_all();
    assert_eq!(lease.term_pin_count(), 0);
}
