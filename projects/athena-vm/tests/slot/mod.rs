//! 稠密槽表合同。

#![allow(unsafe_code)]

use athena_types::TermId;
use athena_vm::{SlotTable, SlotValue};

#[test]
fn dense_get_set_skips_empty() {
    let mut table = SlotTable::new();
    assert!(table.get(0).is_none());
    table.set(2, SlotValue::Term(TermId(7)));
    assert!(table.get(0).is_none());
    assert!(table.get(1).is_none());
    assert_eq!(table.get(2), Some(SlotValue::Term(TermId(7))));
    assert_eq!(table.len(), 3);
}

#[test]
fn unchecked_roundtrip_after_ensure() {
    let mut table = SlotTable::with_capacity(4);
    unsafe {
        table.set_unchecked(1, SlotValue::Boolean(true));
        assert_eq!(table.get_unchecked(1), SlotValue::Boolean(true));
    }
    assert_eq!(table.get(1), Some(SlotValue::Boolean(true)));
}
