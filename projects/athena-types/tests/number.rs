use athena_types::{DiagnosticCode, Number, Result};
use num_rational::BigRational;

#[test]
fn exact_add_rational() {
    let a = Number::rational(BigRational::new(1.into(), 3.into()));
    let b = Number::rational(BigRational::new(1.into(), 6.into()));
    let got: Result<Number> = a.add(b);
    assert_eq!(got.unwrap(), Number::rational(BigRational::new(1.into(), 2.into())));
}

#[test]
fn div_by_zero_is_athena_code() {
    let err = Number::small_int(1).div(Number::small_int(0)).unwrap_err();
    assert_eq!(err.code, DiagnosticCode::DivideByZero);
    assert_eq!(err.code.as_str(), "ATHENA_DIVIDE_BY_ZERO");
}
