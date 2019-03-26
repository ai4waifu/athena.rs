//! Layout / view / broadcast 合同。

use athena_ndarray::{
    ArrayError, ArrayLayout, ArrayViewSpec, Axis, BroadcastSpec, LogicalShape, MemoryBudget, permute_axes,
};

#[test]
fn row_major_strides_and_offsets() {
    let shape = LogicalShape::new([2, 3]).unwrap();
    let layout = ArrayLayout::row_major(shape, 8).unwrap();
    assert_eq!(layout.strides, vec![3, 1]);
    assert_eq!(layout.byte_offset_of_flat(4).unwrap(), 32);
}

#[test]
fn view_stale_on_revision_bump() {
    let shape = LogicalShape::new([4]).unwrap();
    let layout = ArrayLayout::row_major(shape, 8).unwrap();
    let view = ArrayViewSpec::identity(&layout, 1).unwrap();
    assert!(view.ensure_fresh(1).is_ok());
    assert!(matches!(
        view.ensure_fresh(2),
        Err(ArrayError::StaleView { expected: 1, actual: 2 })
    ));
}

#[test]
fn broadcast_align_and_chunked_eval() {
    let a = LogicalShape::new([3, 1]).unwrap();
    let b = LogicalShape::new([1, 4]).unwrap();
    let spec = BroadcastSpec::align(&a, &b).unwrap();
    assert_eq!(spec.out_shape.dimensions(), &[3, 4]);
    let budget = MemoryBudget::new(16).unwrap(); // 2×u64
    let mut sizes = Vec::new();
    spec.for_each_flat_chunk(budget, 8, |_, len| sizes.push(len)).unwrap();
    assert_eq!(sizes, vec![2, 2, 2, 2, 2, 2]);
}

#[test]
fn broadcast_rejects_incompatible() {
    let a = LogicalShape::new([3, 2]).unwrap();
    let b = LogicalShape::new([3, 4]).unwrap();
    assert!(matches!(
        BroadcastSpec::align(&a, &b),
        Err(ArrayError::BroadcastIncompatible)
    ));
}

#[test]
fn permute_axes_reorders_shape() {
    let shape = LogicalShape::new([2, 3, 4]).unwrap();
    let out = permute_axes(&shape, &[Axis(2), Axis(0), Axis(1)]).unwrap();
    assert_eq!(out.dimensions(), &[4, 2, 3]);
}
