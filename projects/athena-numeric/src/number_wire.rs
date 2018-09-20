//! 废弃：旧 `athena_types::wire::WireNumber` 桥。执行路径请用 [`NumericValue`]。
//!
//! 本模块仅保留给尚未迁完的宿主适配；新代码禁止依赖。

use athena_types::{
    Diagnostic, DiagnosticCode, Result,
    wire::{ExactNumber, RealNumber, WireNumber},
};

use crate::{integer::Integer, number::NumericValue, rational::Rational};

/// wire → [`NumericValue`]。
pub fn from_wire(n: &WireNumber) -> Result<NumericValue> {
    match n {
        WireNumber::Exact(ExactNumber::Integer(s)) => Integer::from_decimal_str(s)
            .map(NumericValue::integer)
            .map_err(|_| Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("operation", "from_wire")),
        WireNumber::Exact(ExactNumber::Rational { numer, denom }) => {
            let n = Integer::from_decimal_str(numer)
                .map_err(|_| Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("operation", "from_wire"))?;
            let d = Integer::from_decimal_str(denom)
                .map_err(|_| Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("operation", "from_wire"))?;
            Ok(NumericValue::from_rational_normalized(Rational::try_new(n, d)?))
        }
        WireNumber::Real(RealNumber::Machine(x)) => Ok(NumericValue::machine(*x)),
    }
}

/// [`NumericValue`] → wire（宿主渲染过渡）。
pub fn to_wire(n: &NumericValue) -> WireNumber {
    match n.repr() {
        crate::number::NumericRepr::Integer(i) => WireNumber::Exact(ExactNumber::Integer(i.to_decimal_string())),
        crate::number::NumericRepr::Rational(r) => {
            if r.denominator().is_one() {
                WireNumber::Exact(ExactNumber::Integer(r.numerator().to_decimal_string()))
            }
            else {
                WireNumber::Exact(ExactNumber::Rational {
                    numer: r.numerator().to_decimal_string(),
                    denom: r.denominator().to_decimal_string(),
                })
            }
        }
        crate::number::NumericRepr::Real(crate::real::Real::Machine(x)) => WireNumber::machine(*x),
        _ => WireNumber::Exact(ExactNumber::Integer(n.to_render_string())),
    }
}
