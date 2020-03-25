//! 自 `src/reasoning/mgraph/equivalence/congruence.rs` 迁出的原内联测试。

use athena_engine::{Session, reasoning::mgraph::equivalence::*};
use std::collections::HashMap;

#[test]
fn distinct_moduli_do_not_share_equivalence() {
    let mut index = CongruenceIndex::default();
    index.union(7, 10, 20);
    index.union(11, 10, 30);
    assert_eq!(index.find(7, 10), index.find(7, 20));
    assert_ne!(index.find(7, 10), index.find(7, 30));
    assert_eq!(index.find(11, 10), index.find(11, 30));
    assert_eq!(index.modulus_count(), 2);
}
