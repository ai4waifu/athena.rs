//! `AllocationHeader` 布局合同。

use athena_gc::AllocationHeader;

#[test]
fn header_aligned_8() {
    assert_eq!(AllocationHeader::size() % 8, 0);
    assert!(AllocationHeader::size() >= 24);
}
