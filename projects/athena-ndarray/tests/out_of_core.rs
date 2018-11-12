//! Out-of-core chunk iteration contract tests.

use athena_ndarray::{ArrayStorage, ChunkedArray, LogicalShape, MemoryBudget, StorageCapabilities};

#[derive(Debug)]
struct Store(Vec<u64>);

impl ArrayStorage<u64> for Store {
    type Error = ();

    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities {
            writable: true,
            random_read: true,
            sequential_read: true,
            persistent: false,
        }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<u64>, ()> {
        let start = offset as usize;
        Ok(self.0[start..start + len].to_vec())
    }

    fn write_range(&mut self, offset: u64, values: &[u64]) -> Result<(), ()> {
        let start = offset as usize;
        self.0[start..start + values.len()].copy_from_slice(values);
        Ok(())
    }
}

#[test]
fn shape_overflow_is_structured() {
    assert!(LogicalShape::new([u64::MAX, 2]).is_err());
}

#[test]
fn iteration_is_bounded() {
    let shape = LogicalShape::new([10]).unwrap();
    let array = ChunkedArray::new(shape, Store((0..10).collect()), MemoryBudget::new(24).unwrap()).unwrap();
    let mut sizes = Vec::new();
    array.for_each_chunk(|_, values| sizes.push(values.len())).unwrap();
    assert_eq!(sizes, [3, 3, 3, 1]);
}
