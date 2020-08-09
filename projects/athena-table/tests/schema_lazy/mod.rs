//! Schema / LazyTable / 分块列合同测试。

use athena_ndarray::{ArrayStorage, MemoryBudget, StorageCapabilities};
use athena_table::{Field, LogicalType, Schema, Table, TableExpr, column_from_store};

#[derive(Debug)]
struct Store(Vec<i64>);

impl ArrayStorage<i64> for Store {
    type Error = ();

    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities { writable: false, random_read: true, sequential_read: true, persistent: false }
    }

    fn read_range(&self, offset: u64, len: usize) -> Result<Vec<i64>, ()> {
        let start = offset as usize;
        Ok(self.0[start..start + len].to_vec())
    }

    fn write_range(&mut self, _: u64, _: &[i64]) -> Result<(), ()> {
        Err(())
    }
}

#[test]
fn rejects_duplicate_fields() {
    let err = Schema::new([Field::new("a", LogicalType::Int(64), true), Field::new("a", LogicalType::Utf8, true)]);
    assert!(err.is_err());
}

#[test]
fn lazy_select_limit_preserves_schema() {
    let schema = Schema::new([Field::new("id", LogicalType::Int(64), false), Field::new("name", LogicalType::Utf8, true)]).unwrap();
    let table = Table::with_rows(schema.clone(), 100).lazy().select([TableExpr::Column("id".into())]).limit(10).collect_meta().unwrap();
    assert_eq!(table.schema(), &schema);
    assert_eq!(table.row_count(), 10);
}

#[test]
fn chunked_column_respects_budget() {
    let field = Field::new("x", LogicalType::Int(64), false);
    let col = column_from_store(field, Store((0..10).collect()), MemoryBudget::new(24).unwrap()).unwrap();
    let mut sizes = Vec::new();
    col.for_each_chunk(|_, values| sizes.push(values.len())).unwrap();
    assert_eq!(sizes, [3, 3, 3, 1]);
}
