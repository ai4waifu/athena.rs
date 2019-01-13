//! [`kernel_number`] 合同测试。

use athena_numeric::{NumericValue, add, div};
use athena_types::DiagnosticCode;

#[test]
fn exact_add_rational() {
    let a = NumericValue::rational_i64(1, 3).unwrap();
    let b = NumericValue::rational_i64(1, 6).unwrap();
    let got = add(a, b).unwrap();
    assert_eq!(got, NumericValue::rational_i64(1, 2).unwrap());
}

#[test]
fn div_by_zero_is_athena_code() {
    let err = div(NumericValue::small_int(1), NumericValue::small_int(0)).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::DivideByZero);
    assert_eq!(err.code.as_str(), "ATHENA_DIVIDE_BY_ZERO");
}
