//! `Number` fingerprint 内容哈希合同。

use athena_numeric::{Integer, Number};

#[test]
fn integer_content_hash_is_limb_stable() {
    let a = Number::Integer(Integer::from_i64(42));
    let b = Number::Integer(Integer::from_i64(42));
    let c = Number::Integer(Integer::from_i64(43));
    assert_eq!(a.fingerprint_content_hash(), b.fingerprint_content_hash());
    assert_ne!(a.fingerprint_content_hash(), c.fingerprint_content_hash());
    assert_eq!(a.fingerprint_domain_tag(), 1);
}
