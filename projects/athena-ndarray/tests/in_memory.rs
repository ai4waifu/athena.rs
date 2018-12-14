//! In-memory storage helper tests.

use athena_ndarray::{MemoryBudget, array1d};

#[test]
fn array1d_respects_budget() {
    let a = array1d((0u64..6).collect(), MemoryBudget::new(16).unwrap()).unwrap();
    let mut sizes = Vec::new();
    a.for_each_chunk(|_, chunk| sizes.push(chunk.len())).unwrap();
    assert_eq!(sizes, [2, 2, 2]);
}

#[test]
fn array1d_reads_all_elements() {
    let a = array1d(vec![1u64, 2, 3], MemoryBudget::new(64).unwrap()).unwrap();
    let v = a.read_range(0, 3).unwrap();
    assert_eq!(v, vec![1, 2, 3]);
}
