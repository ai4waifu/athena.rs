//! GT1d/GT1e：typed property / weight columns。

use athena_graph::{
    GraphError, PropertyCell, PropertyColumn, PropertyStore, WeightColumn, WeightDomainTag,
};

#[test]
fn property_column_rejects_length_mismatch() {
    let err = PropertyColumn::<i32>::try_from_dense(3, vec![1, 2]).unwrap_err();
    assert_eq!(err, GraphError::PropertyLengthMismatch { expected: 3, actual: 2 });
}

#[test]
fn property_cell_missing_unknown_distinct() {
    let col = PropertyColumn::try_from_cells(
        3,
        vec![PropertyCell::Present(10), PropertyCell::Missing, PropertyCell::Unknown],
    )
    .unwrap();
    assert_eq!(col.get(0).unwrap().as_present(), Some(&10));
    assert_eq!(col.get(1), Some(&PropertyCell::Missing));
    assert_eq!(col.get(2), Some(&PropertyCell::Unknown));
}

#[test]
fn property_store_binds_node_and_edge_columns() {
    let mut store = PropertyStore::<&'static str, i64>::new();
    store
        .insert_node_column("label", 2, PropertyColumn::try_from_dense(2, vec!["a", "b"]).unwrap())
        .unwrap();
    store
        .insert_edge_column(
            "cap",
            1,
            PropertyColumn::try_from_cells(1, vec![PropertyCell::Present(7)]).unwrap(),
        )
        .unwrap();
    assert_eq!(store.node_column_count(), 1);
    assert_eq!(store.edge_column_count(), 1);
    assert_eq!(store.node_column("label").unwrap().get(1).unwrap().as_present(), Some(&"b"));
    assert_eq!(store.edge_column("cap").unwrap().get(0).unwrap().as_present(), Some(&7));
}

#[test]
fn property_store_rejects_edge_column_wrong_len() {
    let mut store = PropertyStore::<(), i32>::new();
    let err = store
        .insert_edge_column("w", 2, PropertyColumn::try_from_dense(1, vec![1]).unwrap())
        .unwrap_err();
    assert_eq!(err, GraphError::PropertyLengthMismatch { expected: 2, actual: 1 });
}

#[test]
fn weight_column_carries_domain_tag() {
    let w = WeightColumn::try_dense(WeightDomainTag::ExactInteger, 2, vec![1i64, 2]).unwrap();
    assert_eq!(w.domain(), WeightDomainTag::ExactInteger);
    assert_eq!(w.len(), 2);
    assert_eq!(w.column().get(0).unwrap().as_present(), Some(&1));
}

#[test]
fn weight_column_rejects_mismatch() {
    let err = WeightColumn::try_new(
        WeightDomainTag::MachineReal,
        2,
        vec![PropertyCell::Present(1.0f64)],
    )
    .unwrap_err();
    assert_eq!(err, GraphError::PropertyLengthMismatch { expected: 2, actual: 1 });
}
