use athena_graph::CsrGraph;
use athena_ndarray::{ChunkStore, ChunkedArray, LogicalShape, MemoryBudget, StoreCapabilities};
#[derive(Debug)]
struct Store(Vec<u64>);
impl ChunkStore<u64> for Store {
    type Error = ();
    fn len(&self) -> u64 {
        self.0.len() as u64
    }
    fn capabilities(&self) -> StoreCapabilities {
        StoreCapabilities { writable: false, random_read: true, persistent: false }
    }
    fn read_range(&self, o: u64, n: usize) -> Result<Vec<u64>, ()> {
        Ok(self.0[o as usize..o as usize + n].to_vec())
    }
    fn write_range(&mut self, _: u64, _: &[u64]) -> Result<(), ()> {
        Err(())
    }
}
fn arr(v: Vec<u64>, n: usize) -> ChunkedArray<u64, Store> {
    let s = LogicalShape::new([v.len() as u64]).unwrap();
    ChunkedArray::new(s, Store(v), MemoryBudget::new(n * 8).unwrap()).unwrap()
}
#[test]
fn high_degree_node_is_chunked() {
    let g = CsrGraph::new(3, arr(vec![0, 5, 5, 5], 2), arr(vec![1, 2, 1, 2, 1], 2)).unwrap();
    let mut c = vec![];
    g.for_each_neighbor_chunk(0, |v| c.push(v.to_vec())).unwrap();
    assert_eq!(c, vec![vec![1, 2], vec![1, 2], vec![1]]);
}
