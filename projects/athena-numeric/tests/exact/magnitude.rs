//! Magnitude / Nat·Int（`meta + Magnitude`）布局与 canonical 表示测试。

use athena_numeric::{
    NumericContext,
    integer::{Integer, Sign},
    natural::Natural,
};
use std::mem::{align_of, size_of};

#[test]
fn natural_and_integer_size_align_lp64() {
    assert_eq!(size_of::<Natural>(), 24);
    assert_eq!(align_of::<Natural>(), 8);
    assert_eq!(size_of::<Integer>(), 24);
    assert_eq!(align_of::<Integer>(), 8);
}

#[test]
fn integer_sign_in_meta_not_separate_field() {
    let n = Integer::from_i64(-7);
    assert_eq!(n.sign(), Sign::Negative);
    assert_eq!(n.abs().sign(), Sign::Positive);
    assert_eq!(n.neg().to_i64(), Some(7));
    assert_eq!(size_of::<Integer>(), size_of::<Natural>());
    assert!(Integer::zero().is_zero());
    assert_eq!(Integer::from_i64(-1).abs(), Integer::one());
}

#[test]
fn modes_upgrade_downgrade() {
    let z = Natural::zero();
    assert!(z.is_zero());
    assert_eq!(z.as_limbs(), &[0]);

    let a = Natural::from_u64(1);
    assert_eq!(a.as_limbs(), &[1]);

    let b = Natural::from_limbs(vec![0, 1]).unwrap();
    assert_eq!(b.as_limbs(), &[0, 1]);

    let c = Natural::from_limbs(vec![1, 2, 3]).unwrap();
    assert_eq!(c.as_limbs(), &[1, 2, 3]);

    // 尾随零不得进入错误 mode。
    let d = Natural::from_limbs(vec![5, 0, 0]).unwrap();
    assert_eq!(d.as_limbs(), &[5]);

    let e = Natural::from_limbs(vec![0, 0, 0]).unwrap();
    assert!(e.is_zero());
    assert_eq!(e.as_limbs(), &[0]);
}

#[test]
fn as_limbs_uses_checked_decode_for_heap() {
    // Heap values must round-trip through decode_magnitude (capacity clamp, no panic).
    let n = Natural::from_limbs(vec![1, 2, 3, 4]).unwrap();
    assert_eq!(n.as_limbs(), &[1, 2, 3, 4]);
    let m = n.try_clone_in(&NumericContext::portable_default()).unwrap();
    assert_eq!(m.as_limbs(), n.as_limbs());
}

#[test]
fn clone_drop_heap() {
    let a = Natural::from_limbs(vec![1, 2, 3, 4]).unwrap();
    let b = a.try_clone_in(&NumericContext::portable_default()).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.as_limbs(), &[1, 2, 3, 4]);
    assert_ne!(a.as_limbs().as_ptr(), b.as_limbs().as_ptr(), "Heap try_clone_in is deep copy");
    drop(a);
    assert_eq!(b.as_limbs(), &[1, 2, 3, 4]);
    let s = b.to_decimal_string();
    assert!(!s.is_empty() && s.chars().all(|c| c.is_ascii_digit()));
}

#[test]
fn clone_inline_limb_only_rejects_heap() {
    let limb1 = Natural::from_u64(7);
    let limb2 = Natural::from_limbs(vec![1, 2]).unwrap();
    let heap = Natural::from_limbs(vec![1, 2, 3, 4]).unwrap();
    assert_eq!(limb1.clone_inline().expect("Limb1").as_limbs(), &[7]);
    assert_eq!(limb2.clone_inline().expect("Limb2").as_limbs(), &[1, 2]);
    assert!(heap.clone_inline().is_none(), "Living 19: Heap has no clone_inline");

    let i_limb = Integer::from_i64(-3);
    let i_heap = Integer::from_limbs_in(&NumericContext::portable_default(), vec![1, 2, 3, 4]).unwrap();
    assert_eq!(i_limb.clone_inline().expect("signed Limb1").to_i64(), Some(-3));
    assert!(i_heap.clone_inline().is_none());
}

#[test]
fn arithmetic_still_works_across_modes() {
    let a = Natural::from_u64(u64::MAX);
    let b = Natural::from_u64(2);
    let s = a.add(&b);
    assert_eq!(s.as_limbs(), &[1, 1]);

    let p = a.mul(&a);
    assert_eq!(p.as_limbs().len(), 2);

    let h = Natural::from_limbs(vec![u64::MAX, u64::MAX, u64::MAX]).unwrap();
    let q = h.add(&Natural::one());
    assert_eq!(q.as_limbs(), &[0, 0, 0, 1]);
}

#[test]
fn eq_ord_hash_ignore_repr() {
    use std::collections::HashSet;
    let a = Natural::from_limbs(vec![7]).unwrap();
    let b = Natural::from_u64(7);
    assert_eq!(a, b);
    assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);

    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

#[test]
fn limb1_add_stays_inline() {
    let a = Natural::from_u64(3);
    let b = Natural::from_u64(5);
    let s = a.add(&b);
    assert_eq!(s.as_limbs(), &[8]);
    // len==1 ⇒ Limb1，非 Heap
    assert_eq!(s.as_limbs().len(), 1);

    let overflow = Natural::from_u64(u64::MAX).add(&Natural::one());
    assert_eq!(overflow.as_limbs(), &[0, 1]);
    // len==2 ⇒ Limb2，非 Heap
    assert_eq!(overflow.as_limbs().len(), 2);
}

#[test]
fn limb1_mul_product_stays_limb2() {
    let a = Natural::from_u64(u64::MAX);
    let p = a.mul(&a);
    // (2^64-1)^2 = 2^128 - 2^65 + 1 → 恰好 2 limbs
    assert_eq!(p.as_limbs().len(), 2);
    assert_eq!(p.as_limbs(), &[1, u64::MAX - 1]);
}

#[test]
fn limb2_add_carry_upgrades_to_heap() {
    let a = Natural::from_limbs(vec![u64::MAX, u64::MAX]).unwrap();
    let b = Natural::one();
    let s = a.add(&b);
    assert_eq!(s.as_limbs(), &[0, 0, 1]);
    assert_eq!(s.as_limbs().len(), 3);
}

#[test]
fn limb2_mul_fixed_width() {
    let a = Natural::from_limbs(vec![u64::MAX, 1]).unwrap();
    let b = Natural::from_u64(2);
    let p = a.mul(&b);
    assert_eq!(p.as_limbs(), &[u64::MAX - 1, 3]);
    assert_eq!(p.as_limbs().len(), 2);

    let c = Natural::from_limbs(vec![u64::MAX, u64::MAX]).unwrap();
    let q = c.mul(&c);
    assert!(q.as_limbs().len() >= 3);
    assert!(q.as_limbs().len() <= 4);
}

#[test]
fn fixed_width_commutative() {
    let cases = [
        (Natural::from_u64(0), Natural::from_u64(9)),
        (Natural::from_u64(u64::MAX), Natural::from_u64(u64::MAX)),
        (Natural::from_limbs(vec![1, 1]).unwrap(), Natural::from_limbs(vec![2, 3]).unwrap()),
        (Natural::from_limbs(vec![u64::MAX, u64::MAX]).unwrap(), Natural::from_u64(u64::MAX)),
    ];
    for (a, b) in cases {
        assert_eq!(a.add(&b), b.add(&a));
        assert_eq!(a.mul(&b), b.mul(&a));
    }
}
