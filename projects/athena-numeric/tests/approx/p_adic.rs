//! `ℚ_p` 固定精度算术。

use athena_numeric::{NumericContext, Integer, NumericValue, PAdicValue, PrecisionKind, Rational};

#[test]
fn padic_add_mul_inv_mod_five() {
    let p = Integer::from_i64(5);
    let a = PAdicValue::from_integer(&2.into(), p.try_clone_in(&NumericContext::portable_default()).unwrap(), 4).unwrap();
    let b = PAdicValue::from_integer(&3.into(), p.try_clone_in(&NumericContext::portable_default()).unwrap(), 4).unwrap();
    let sum = a.add(&b).unwrap();
    assert_eq!(sum, PAdicValue::from_integer(&5.into(), p.try_clone_in(&NumericContext::portable_default()).unwrap(), 4).unwrap());
    assert!(!sum.is_zero());

    let zero = a.add(&PAdicValue::from_integer(&(-2).into(), p.try_clone_in(&NumericContext::portable_default()).unwrap(), 4).unwrap()).unwrap();
    assert!(zero.is_zero());

    let prod = a.mul(&b).unwrap();
    assert_eq!(prod, PAdicValue::from_integer(&6.into(), p.try_clone_in(&NumericContext::portable_default()).unwrap(), 4).unwrap());

    let inv = a.inv().unwrap();
    let one = a.mul(&inv).unwrap();
    assert_eq!(one, PAdicValue::from_integer(&1.into(), p, 4).unwrap());
}

#[test]
fn padic_from_rational_and_validate() {
    let p = Integer::from_i64(5);
    let r = Rational::new(Integer::from_i64(2), Integer::from_i64(3));
    let v = PAdicValue::from_rational(&r, p.try_clone_in(&NumericContext::portable_default()).unwrap(), 3).unwrap();
    assert!(v.is_unit());
    let nv = NumericValue::PAdic(v.try_clone_in(&NumericContext::portable_default()).unwrap());
    nv.validate().unwrap();
    assert_eq!(nv.precision().kind, PrecisionKind::Arbitrary);

    assert!(PAdicValue::from_rational(&Rational::new(1.into(), 5.into()), p, 3).is_err());
}

#[test]
fn padic_rejects_composite_prime() {
    assert!(PAdicValue::from_integer(&1.into(), Integer::from_i64(4), 2).is_err());
    assert!(PAdicValue::try_new(Integer::from_i64(5), 0, vec![]).is_err());
}
