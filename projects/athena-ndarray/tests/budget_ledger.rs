//! 预算四轴 enforce 与禁止静默整表物化。

use athena_ndarray::{
    array1d, ArrayError, ArrayStorage, BudgetLedger, ChunkedArray, LogicalShape, MemoryBudget, StorageCapabilities,
};

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
fn open_chunks_axis_is_enforced() {
    let budget = MemoryBudget::detailed(1024, 1024, 1024, 1).unwrap();
    let mut ledger = BudgetLedger::new();
    ledger.open_chunk(budget).unwrap();
    assert!(matches!(
        ledger.open_chunk(budget),
        Err(ArrayError::OpenChunksExceeded { .. })
    ));
    ledger.close_chunk();
    let shape = LogicalShape::new([8]).unwrap();
    let array = ChunkedArray::new(shape, Store((0..8).collect()), budget).unwrap();
    assert!(array.read_range(0, 1).is_ok());
    assert_eq!(array.ledger_snapshot().open_chunks, 0);
}

#[test]
fn scratch_and_spill_axes_are_enforced() {
    let budget = MemoryBudget::detailed(64, 16, 32, 4).unwrap();
    let shape = LogicalShape::new([2]).unwrap();
    let array = ChunkedArray::new(shape, Store(vec![1, 2]), budget).unwrap();
    array.acquire_scratch(16).unwrap();
    assert!(matches!(
        array.acquire_scratch(1),
        Err(ArrayError::ScratchBudgetExceeded { .. })
    ));
    array.release_scratch(16);
    array.acquire_spill(32).unwrap();
    assert!(matches!(
        array.acquire_spill(1),
        Err(ArrayError::SpillBudgetExceeded { .. })
    ));
}

#[test]
fn array1d_rejects_over_budget_full_materialize() {
    let budget = MemoryBudget::new(8).unwrap(); // 1×u64
    let err = array1d(vec![1u64, 2, 3], budget).unwrap_err();
    assert!(matches!(err, ArrayError::FullMaterializeForbidden { .. }));
}

#[test]
fn try_full_view_rejects_over_budget() {
    let budget = MemoryBudget::new(8).unwrap();
    let shape = LogicalShape::new([4]).unwrap();
    let data = [1u64, 2, 3, 4];
    let err = ChunkedArray::<u64, Store>::try_full_view(budget, &shape, &data).unwrap_err();
    assert!(matches!(err, ArrayError::FullMaterializeForbidden { .. }));
}

#[test]
fn for_each_chunk_never_loads_whole_table_at_once() {
    let budget = MemoryBudget::new(24).unwrap(); // 3×u64
    let shape = LogicalShape::new([10]).unwrap();
    let array = ChunkedArray::new(shape, Store((0..10).collect()), budget).unwrap();
    let mut max_chunk = 0;
    array
        .for_each_chunk(|_, values| max_chunk = max_chunk.max(values.len()))
        .unwrap();
    assert!(max_chunk <= 3);
    assert_eq!(array.ledger_snapshot().open_chunks, 0);
    assert_eq!(array.ledger_snapshot().used_resident, 0);
}
