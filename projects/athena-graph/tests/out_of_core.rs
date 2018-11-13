//! CSR out-of-core neighbor streaming tests.

use athena_graph::CsrGraph;
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
            writable: false,
            random_read: true,
            sequential_read: true,
            persistent: false,
        }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<u64>, ()> {
        let start = offset as usize;
        Ok(self.0[start..start + len].to_vec())
    }

    fn write_range(&mut self, _: u64, _: &[u64]) -> Result<(), ()> {
        Err(())
    }
}

fn arr(values: Vec<u64>, budget_elements: usize) -> ChunkedArray<u64, Store> {
    let shape = LogicalShape::new([values.len() as u64]).unwrap();
    ChunkedArray::new(shape, Store(values), MemoryBudget::new(budget_elements * 8).unwrap()).unwrap()
}

#[test]
fn high_degree_node_is_chunked() {
    let graph = CsrGraph::new(3, arr(vec![0, 5, 5, 5], 2), arr(vec![1, 2, 1, 2, 1], 2)).unwrap();
    let mut chunks = Vec::new();
    graph.for_each_neighbor_chunk(0, |values| chunks.push(values.to_vec())).unwrap();
    assert_eq!(chunks, vec![vec![1, 2], vec![1, 2], vec![1]]);
}
