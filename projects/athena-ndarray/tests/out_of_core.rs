use athena_ndarray::{ChunkStore, ChunkedArray, LogicalShape, MemoryBudget, StoreCapabilities};
#[derive(Debug)]
struct Store(Vec<u64>);
impl ChunkStore<u64> for Store {
    type Error = ();
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn capabilities(&self) -> StoreCapabilities {
        StoreCapabilities { writable: true, random_read: true, persistent: false }
    }
    fn read_range(&self, o: u64, n: usize) -> Result<Vec<u64>, ()> {
        Ok(self.0[o as usize..o as usize + n].to_vec())
    }
    fn write_range(&mut self, o: u64, v: &[u64]) -> Result<(), ()> {
        self.0[o as usize..o as usize + v.len()].copy_from_slice(v);
        Ok(())
    }
}
#[test]
fn iteration_is_bounded() {
    let shape = LogicalShape::new([10]).unwrap();
    let array = ChunkedArray::new(shape, Store((0..10).collect()), MemoryBudget::new(24).unwrap()).unwrap();
    let mut sizes = vec![];
    array.for_each_chunk(|_, v| sizes.push(v.len())).unwrap();
    assert_eq!(sizes, [3, 3, 3, 1]);
}
