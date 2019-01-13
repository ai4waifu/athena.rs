//! Wire 解码：`athena_types::wire::WireNumber` → [`NumericValue`]。
//!
//! SXO / 宿主 frontend 负责源文本 parse → wire。数值内核只接受已规范化的 wire 载荷。

use athena_types::{
    Diagnostic, DiagnosticCode, Result,
    wire::{ExactNumber, RealNumber, WireNumber},
};
use std::str::FromStr;

use crate::{integer::Integer, number::NumericValue, rational::Rational};

/// 将宿主 wire 解码为执行用 [`NumericValue`]。
pub fn from_wire(n: &WireNumber) -> Result<NumericValue> {
    match n {
        WireNumber::Exact(ExactNumber::Integer(s)) => decode_wire_integer(s).map(NumericValue::integer),
        WireNumber::Exact(ExactNumber::Rational { numer, denom }) => {
            let n = decode_wire_integer(numer)?;
            let d = decode_wire_integer(denom)?;
            Ok(NumericValue::from_rational_normalized(Rational::try_new(n, d)?))
        }
        WireNumber::Real(RealNumber::Machine(x)) => Ok(NumericValue::machine(*x)),
    }
}

/// [`NumericValue`] → wire（宿主渲染 / 序列化过渡）。
pub fn to_wire(n: &NumericValue) -> WireNumber {
    match n {
        NumericValue::Integer(i) => WireNumber::Exact(ExactNumber::Integer(i.to_decimal_string())),
        NumericValue::Rational(r) => {
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
        NumericValue::Real(crate::real::Real::Machine(x)) => WireNumber::machine(*x),
        _ => WireNumber::Exact(ExactNumber::Integer(n.to_render_string())),
    }
}

fn decode_wire_integer(s: &str) -> Result<Integer> {
    let payload_len = s.len() as u32;
    if payload_len > crate::backends::PURE_RUST_WIRE_PAYLOAD_LIMIT_BYTES {
        return Err(Diagnostic::new(DiagnosticCode::NumericConversionForbidden)
            .detail("domain", "numeric")
            .detail("operation", "wire_payload_limit"));
    }
    Integer::from_str(s).map_err(|_| {
        Diagnostic::new(DiagnosticCode::NumericConversionForbidden).detail("domain", "numeric").detail("operation", "from_wire")
    })
}
