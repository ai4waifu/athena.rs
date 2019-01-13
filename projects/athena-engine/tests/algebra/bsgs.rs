//! BSGS 链冒烟测试（自 `src/algebra/bsgs.rs` 迁出）。

use athena_engine::{BsgsChain, Integer, algebra::RawPerm};

#[test]
fn symmetric_group_s3_order() {
    let a = RawPerm::new(vec![1, 0, 2], 3).unwrap();
    let b = RawPerm::new(vec![1, 2, 0], 3).unwrap();
    let chain = BsgsChain::from_generators(&[a, b], 3);
    assert_eq!(chain.order, Integer::from_i64(6));
}

#[test]
fn klein_four_order() {
    let a = RawPerm::new(vec![1, 0, 3, 2], 4).unwrap();
    let b = RawPerm::new(vec![2, 3, 0, 1], 4).unwrap();
    let chain = BsgsChain::from_generators(&[a, b], 4);
    assert_eq!(chain.order, Integer::from_i64(4));
}

#[test]
fn compose_follows_right_to_left_on_points() {
    let p = RawPerm::new(vec![1, 0, 2], 3).unwrap();
    let q = RawPerm::new(vec![1, 2, 0], 3).unwrap();
    let pq = p.compose(&q).unwrap();
    assert_eq!(pq.apply(0), p.apply(q.apply(0)));
}
