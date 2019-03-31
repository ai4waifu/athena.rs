//! 内存存储辅助测试。

use athena_ndarray::{ArrayStorage, ChunkedArray, LogicalShape, MemoryBudget, StorageCapabilities, array1d};

#[derive(Debug)]
struct Store(Vec<u64>);

impl ArrayStorage<u64> for Store {
    type Error = ();

    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities { writable: false, random_read: true, sequential_read: true, persistent: false }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<u64>, ()> {
        let start = offset as usize;
        Ok(self.0[start..start + len].to_vec())
    }

    fn write_range(&mut self, _: u64, _: &[u64]) -> Result<(), ()> {
        Err(())
    }
}

#[test]
fn chunked_iteration_respects_resident_budget() {
    // 超预算数据允许绑定 storage；迭代按 chunk，禁止一次整表。
    let budget = MemoryBudget::new(16).unwrap(); // 2×u64
    let shape = LogicalShape::new([6]).unwrap();
    let a = ChunkedArray::new(shape, Store((0u64..6).collect()), budget).unwrap();
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
